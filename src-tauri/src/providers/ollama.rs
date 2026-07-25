use super::{
    ConfigApplied, GpuOffload, InstalledModel, ProviderClient, ProviderError, PullProgress,
    PullStatus,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::mpsc::Sender;

pub struct OllamaClient {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaClient {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .expect("failed to build reqwest client");
        OllamaClient { base_url, client }
    }
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<TagModel>,
}

#[derive(Debug, Deserialize)]
struct TagModel {
    name: String,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PullLine {
    status: Option<String>,
    total: Option<u64>,
    completed: Option<u64>,
    error: Option<String>,
}

impl PullLine {
    fn into_progress(self) -> PullProgress {
        if let Some(error) = self.error {
            return PullProgress {
                status: PullStatus::Error,
                downloaded_bytes: None,
                total_bytes: None,
                message: Some(error),
            };
        }
        let status_text = self.status.unwrap_or_default();
        let status = if status_text == "success" {
            PullStatus::Success
        } else if status_text.starts_with("verifying") || status_text.starts_with("writing") {
            PullStatus::Verifying
        } else {
            PullStatus::Downloading
        };
        PullProgress {
            status,
            downloaded_bytes: self.completed,
            total_bytes: self.total,
            message: Some(status_text),
        }
    }
}

#[async_trait]
impl ProviderClient for OllamaClient {
    async fn health_check(&self) -> Result<(), ProviderError> {
        let resp = self
            .client
            .get(format!("{}/api/tags", self.base_url))
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
            .get(format!("{}/api/tags", self.base_url))
            .send()
            .await?;
        let parsed: TagsResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::ParseError(e.to_string()))?;
        Ok(parsed
            .models
            .into_iter()
            .map(|m| InstalledModel {
                name: m.name,
                size_bytes: m.size,
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
            .post(format!("{}/api/pull", self.base_url))
            .json(&serde_json::json!({ "model": identifier, "stream": true }))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(ProviderError::RequestFailed(format!(
                "pull failed with status {}",
                resp.status()
            )));
        }

        let mut stream = resp.bytes_stream();
        let mut buf: Vec<u8> = Vec::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buf.extend_from_slice(&chunk);

            while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
                let line_bytes: Vec<u8> = buf.drain(..=pos).collect();
                let line = String::from_utf8_lossy(&line_bytes);
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                let parsed: PullLine = serde_json::from_str(line)
                    .map_err(|e| ProviderError::ParseError(e.to_string()))?;
                let is_error = parsed.error.is_some();
                let pp = parsed.into_progress();
                let _ = progress.send(pp).await;
                if is_error {
                    return Err(ProviderError::RequestFailed(
                        "ollama reported an error while pulling".to_string(),
                    ));
                }
            }
        }

        Ok(())
    }

    async fn configure_model(
        &self,
        _model: &str,
        context_length: Option<u32>,
        gpu_offload: Option<GpuOffload>,
    ) -> Result<ConfigApplied, ProviderError> {
        // Ollama has no "save config" endpoint — num_ctx/num_gpu are passed
        // per request to /api/chat (wired in the chat-messaging feature).
        Ok(ConfigApplied {
            context_length_applied: context_length,
            gpu_offload_applied: gpu_offload.map(|g| g.to_value_string()),
            requires_reload: false,
            note: Some(
                "Ollama applies context/GPU settings per chat request, not on save".to_string(),
            ),
        })
    }
}
