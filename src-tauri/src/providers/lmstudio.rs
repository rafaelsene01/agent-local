use super::{HEALTH_CHECK_TIMEOUT, 
    ConfigApplied, GpuOffload, InstalledModel, ProviderClient, ProviderError, PullProgress,
    PullStatus,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

pub struct LmStudioClient {
    base_url: String,
    client: reqwest::Client,
}

impl LmStudioClient {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build reqwest client");
        LmStudioClient { base_url, client }
    }
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    models: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    key: String,
    size_bytes: Option<u64>,
}

// SPEC_DEVIATION: design.md guessed camelCase `contextLength`/`gpuOffload` as
// a graduated value. The real v1 REST API (confirmed against
// lmstudio.ai/docs/developer/rest/{load,download,download-status} during T6)
// uses snake_case `context_length` and a boolean `offload_kv_cache_to_gpu` —
// there is no partial/fractional GPU offload, only on/off.
#[derive(Debug, Deserialize)]
struct DownloadStartResponse {
    job_id: Option<String>,
    status: String,
    total_size_bytes: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct DownloadStatusResponse {
    status: String,
    total_size_bytes: Option<u64>,
    downloaded_bytes: Option<u64>,
}

#[async_trait]
impl ProviderClient for LmStudioClient {
    async fn health_check(&self) -> Result<(), ProviderError> {
        let resp = self
            .client
            .get(format!("{}/api/v1/models", self.base_url))
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
            .get(format!("{}/api/v1/models", self.base_url))
            .send()
            .await?;
        let parsed: ModelsResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::ParseError(e.to_string()))?;
        Ok(parsed
            .models
            .into_iter()
            .map(|m| InstalledModel {
                name: m.key,
                size_bytes: m.size_bytes,
            })
            .collect())
    }

    async fn pull_model(
        &self,
        identifier: &str,
        progress: Sender<PullProgress>,
    ) -> Result<(), ProviderError> {
        let resp = self
            .client
            .post(format!("{}/api/v1/models/download", self.base_url))
            .json(&serde_json::json!({ "model": identifier }))
            .send()
            .await?;
        let start: DownloadStartResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::ParseError(e.to_string()))?;

        if start.status == "already_downloaded" {
            let _ = progress
                .send(PullProgress {
                    status: PullStatus::Success,
                    downloaded_bytes: start.total_size_bytes,
                    total_bytes: start.total_size_bytes,
                    message: Some("already_downloaded".to_string()),
                })
                .await;
            return Ok(());
        }

        let job_id = start.job_id.ok_or_else(|| {
            ProviderError::ParseError("missing job_id in download response".to_string())
        })?;

        loop {
            let resp = self
                .client
                .get(format!(
                    "{}/api/v1/models/download/status/{}",
                    self.base_url, job_id
                ))
                .send()
                .await?;
            let status: DownloadStatusResponse = resp
                .json()
                .await
                .map_err(|e| ProviderError::ParseError(e.to_string()))?;

            let pull_status = match status.status.as_str() {
                "completed" => PullStatus::Success,
                "failed" => PullStatus::Error,
                _ => PullStatus::Downloading,
            };
            let is_done = pull_status == PullStatus::Success;
            let is_error = pull_status == PullStatus::Error;

            let _ = progress
                .send(PullProgress {
                    status: pull_status,
                    downloaded_bytes: status.downloaded_bytes,
                    total_bytes: status.total_size_bytes,
                    message: Some(status.status.clone()),
                })
                .await;

            if is_done {
                return Ok(());
            }
            if is_error {
                return Err(ProviderError::RequestFailed(
                    "lm studio reported a failed download".to_string(),
                ));
            }

            tokio::time::sleep(Duration::from_millis(750)).await;
        }
    }

    async fn configure_model(
        &self,
        model: &str,
        context_length: Option<u32>,
        gpu_offload: Option<GpuOffload>,
    ) -> Result<ConfigApplied, ProviderError> {
        let offload_bool = gpu_offload.as_ref().map(|g| !matches!(g, GpuOffload::Off));

        let mut body = serde_json::json!({
            "model": model,
            "echo_load_config": true,
        });
        if let Some(ctx) = context_length {
            body["context_length"] = serde_json::json!(ctx);
        }
        if let Some(offload) = offload_bool {
            body["offload_kv_cache_to_gpu"] = serde_json::json!(offload);
        }

        let resp = self
            .client
            .post(format!("{}/api/v1/models/load", self.base_url))
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ProviderError::RequestFailed(format!(
                "load failed with status {}",
                resp.status()
            )));
        }

        let note = if matches!(gpu_offload, Some(GpuOffload::Fraction(_))) {
            Some(
                "LM Studio only supports on/off GPU KV-cache offload, not a partial fraction — treated as fully on"
                    .to_string(),
            )
        } else {
            None
        };

        Ok(ConfigApplied {
            context_length_applied: context_length,
            gpu_offload_applied: offload_bool.map(|b| b.to_string()),
            requires_reload: true,
            note,
        })
    }
}
