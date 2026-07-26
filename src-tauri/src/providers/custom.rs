use super::openai_stream;
use super::{ChatMessage, ChatStream, HEALTH_CHECK_TIMEOUT, SHORT_REQUEST_TIMEOUT, ConfigApplied, GpuOffload, InstalledModel, ModelLimits, ProviderClient, ProviderError, PullProgress};
use async_trait::async_trait;
use serde::Deserialize;
use tokio::sync::mpsc::Sender;

/// Generic client for a manually-added "OpenAI-compatible" server (CONN-01
/// AC4). Only the OpenAI-standard `GET /v1/models` is universal across such
/// servers — there's no standard pull/configure API, so those two report
/// clearly that they aren't supported instead of pretending to work.
pub struct CustomClient {
    base_url: String,
    client: reqwest::Client,
}

impl CustomClient {
    pub fn new(base_url: String) -> Self {
        let client = super::http_client();
        CustomClient { base_url, client }
    }
}

#[derive(Debug, Deserialize)]
struct ModelsListResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    /// llama.cpp's `/v1/models` adds this block; other OpenAI-compatible
    /// servers don't, hence the Option. Verified live against `llama-server`
    /// on 2026-07-25: `meta.n_ctx_train` = 131072 for Phi-3.5 Mini (the model's
    /// trained window) and `meta.n_ctx` = 21760 (what it allocated).
    meta: Option<ModelMeta>,
}

#[derive(Debug, Deserialize)]
struct ModelMeta {
    n_ctx: Option<u32>,
    n_ctx_train: Option<u32>,
}

#[async_trait]
impl ProviderClient for CustomClient {
    async fn health_check(&self) -> Result<(), ProviderError> {
        let resp = self
            .client
            .get(format!("{}/v1/models", self.base_url))
            .timeout(HEALTH_CHECK_TIMEOUT)
            .send()
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(ProviderError::Unavailable)
        }
    }

    async fn list_installed_models(&self) -> Result<Vec<InstalledModel>, ProviderError> {
        let resp = self
            .client
            .get(format!("{}/v1/models", self.base_url))
            .timeout(SHORT_REQUEST_TIMEOUT)
            .send()
            .await?;
        let parsed: ModelsListResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::ParseError(e.to_string()))?;
        Ok(parsed
            .data
            .into_iter()
            .map(|m| InstalledModel {
                name: m.id,
                size_bytes: None,
            })
            .collect())
    }

    /// Matches by the exact id first and falls back to a suffix match: the
    /// embedded runtime knows its models by file name while `llama-server`
    /// reports the full path it was started with.
    async fn model_limits(&self, model: &str) -> Result<ModelLimits, ProviderError> {
        let resp = self
            .client
            .get(format!("{}/v1/models", self.base_url))
            .timeout(SHORT_REQUEST_TIMEOUT)
            .send()
            .await?;
        let parsed: ModelsListResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::ParseError(e.to_string()))?;

        let entry = parsed
            .data
            .iter()
            .find(|m| m.id == model)
            .or_else(|| parsed.data.iter().find(|m| m.id.ends_with(model)))
            .or_else(|| parsed.data.first());

        Ok(entry
            .and_then(|m| m.meta.as_ref())
            .map(|meta| ModelLimits {
                max_context: meta.n_ctx_train,
                current_context: meta.n_ctx,
            })
            .unwrap_or_default())
    }

    async fn pull_model(
        &self,
        _identifier: &str,
        _progress: Sender<PullProgress>,
    ) -> Result<(), ProviderError> {
        Err(ProviderError::RequestFailed(
            "downloading models isn't supported for custom connections".to_string(),
        ))
    }

    async fn configure_model(
        &self,
        _model: &str,
        _context_length: Option<u32>,
        _gpu_offload: Option<GpuOffload>,
    ) -> Result<ConfigApplied, ProviderError> {
        Ok(ConfigApplied {
            context_length_applied: None,
            gpu_offload_applied: None,
            requires_reload: false,
            note: Some(
                "custom connections don't expose a standard config API — context/GPU settings weren't applied"
                    .to_string(),
            ),
        })
    }

    /// A custom connection is OpenAI-compatible by definition (CONN-01 AC4).
    async fn stream_chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        context_length: Option<u32>,
        _gpu_offload: Option<GpuOffload>,
    ) -> Result<ChatStream, ProviderError> {
        openai_stream::stream_chat_completions(
            &self.client,
            &self.base_url,
            model,
            messages,
            context_length,
        )
        .await
    }

}
