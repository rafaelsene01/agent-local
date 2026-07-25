use super::openai_stream;
use super::{ChatMessage, ChatStream, HEALTH_CHECK_TIMEOUT, ConfigApplied, GpuOffload, InstalledModel, ProviderClient, ProviderError, PullProgress};
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;
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
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build reqwest client");
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
