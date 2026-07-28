//! The one client left (SELF-03).
//!
//! `CustomClient` and `EmbeddedClient` were two wrappers around the same
//! server: llama.cpp's `llama-server`, which speaks OpenAI-compatible HTTP.
//! With Ollama and LM Studio gone there is exactly one thing to talk to, so
//! there is no trait and no `Box<dyn>` — the indirection existed to pick
//! between implementations that no longer exist.
//!
//! Two behaviours are carried over deliberately, because both were learned the
//! hard way: models are listed by reading the folder rather than `/v1/models`
//! (which only reports the single loaded model, without a size), and the
//! context window comes from `meta.n_ctx_train` (AD-029/AD-033).

use super::openai_stream;
use super::{
    ChatMessage, ChatStream, InstalledModel, ModelLimits, ProviderError, SHORT_REQUEST_TIMEOUT,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub struct LlamaServerClient {
    /// `None` when no sidecar is running: asking anything of it is
    /// `Unavailable`, which the UI already knows how to show (EMBED-08).
    port: Option<u16>,
    models_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ModelsListResponse {
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
    /// Verified live against `llama-server` (AD-029): `n_ctx_train` = 131072
    /// for Phi-3.5 Mini (the trained window) and `n_ctx` = 21760 (allocated).
    meta: Option<ModelMeta>,
}

#[derive(Debug, Deserialize)]
struct ModelMeta {
    n_ctx: Option<u32>,
    n_ctx_train: Option<u32>,
}

impl LlamaServerClient {
    pub fn new(port: Option<u16>, models_dir: PathBuf) -> Self {
        LlamaServerClient { port, models_dir }
    }

    fn base_url(&self) -> Result<String, ProviderError> {
        let port = self.port.ok_or(ProviderError::Unavailable)?;
        Ok(format!("http://127.0.0.1:{port}"))
    }

    // `health_check` used to live here. Nothing in the app called it once the
    // Conexões screen went (AD-042) — `runtime_status` answers from the database
    // row and the child process, without a request. Its only caller was the test
    // below, which now makes the same point through `model_limits` (C-11).

    /// Installed = the GGUF files in the models folder, not the one the running
    /// server happens to have loaded. `/v1/models` would only ever report that
    /// one, and carries no size — the directory answers both "what can I switch
    /// to" and "how big is it".
    pub fn list_installed_models(&self) -> Vec<InstalledModel> {
        installed_from_disk(&self.models_dir)
    }

    /// Only a running sidecar knows the trained window: it comes from the GGUF
    /// header it loaded, so a stopped runtime reports nothing rather than a
    /// guess. The prompt budget depends on this being honest (AD-033).
    pub async fn model_limits(&self, model: &str) -> Result<ModelLimits, ProviderError> {
        let resp = super::http_client()
            .get(format!("{}/v1/models", self.base_url()?))
            .timeout(SHORT_REQUEST_TIMEOUT)
            .send()
            .await?;
        let parsed: ModelsListResponse = resp
            .json()
            .await
            .map_err(|e| ProviderError::ParseError(e.to_string()))?;

        Ok(select_limits(&parsed.data, model))
    }

    pub async fn stream_chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        context_length: Option<u32>,
    ) -> Result<ChatStream, ProviderError> {
        openai_stream::stream_chat_completions(
            &super::http_client(),
            &self.base_url()?,
            model,
            messages,
            context_length,
        )
        .await
    }
}

/// Split out as a free function so both the client and the tests can use it
/// without a port or a server.
fn installed_from_disk(models_dir: &Path) -> Vec<InstalledModel> {
    let Ok(entries) = std::fs::read_dir(models_dir) else {
        return Vec::new();
    };
    let mut models: Vec<InstalledModel> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let is_gguf = path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"));
            if !is_gguf {
                return None;
            }
            Some(InstalledModel {
                name: path.file_name()?.to_string_lossy().to_string(),
                size_bytes: entry.metadata().ok().map(|meta| meta.len()),
            })
        })
        .collect();
    models.sort_by(|a, b| a.name.cmp(&b.name));
    models
}

/// Matches the exact id first, then a suffix: the app knows models by file
/// name while `llama-server` reports the full path it was started with.
fn select_limits(entries: &[ModelEntry], model: &str) -> ModelLimits {
    entries
        .iter()
        .find(|m| m.id == model)
        .or_else(|| entries.iter().find(|m| m.id.ends_with(model)))
        .or_else(|| entries.first())
        .and_then(|m| m.meta.as_ref())
        .map(|meta| ModelLimits {
            max_context: meta.n_ctx_train,
            current_context: meta.n_ctx,
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, train: Option<u32>, ctx: Option<u32>) -> ModelEntry {
        ModelEntry {
            id: id.to_string(),
            meta: Some(ModelMeta {
                n_ctx: ctx,
                n_ctx_train: train,
            }),
        }
    }

    #[test]
    fn installed_models_are_the_gguf_files_with_their_sizes() {
        let dir = std::env::temp_dir().join(format!("localmind-llama-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Phi-3.5-mini-instruct-Q4_K_M.gguf"), vec![0u8; 2048]).unwrap();
        std::fs::write(dir.join("Qwen2.5-1.5B-Instruct-Q4_K_M.GGUF"), vec![0u8; 4096]).unwrap();
        // The embedding model cache shares this folder — it must not show up.
        std::fs::write(dir.join("model.onnx"), vec![0u8; 10]).unwrap();

        let models = installed_from_disk(&dir);

        assert_eq!(models.len(), 2, "only .gguf files are models");
        assert_eq!(models[0].name, "Phi-3.5-mini-instruct-Q4_K_M.gguf");
        assert_eq!(models[0].size_bytes, Some(2048));
        assert_eq!(models[1].size_bytes, Some(4096), "uppercase .GGUF counts too");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_models_folder_lists_nothing_instead_of_failing() {
        let dir = std::env::temp_dir().join("localmind-llama-does-not-exist");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(installed_from_disk(&dir).is_empty());
    }

    #[test]
    fn limits_come_from_the_trained_window_not_the_allocated_one() {
        let entries = vec![entry("phi.gguf", Some(131072), Some(21760))];
        let limits = select_limits(&entries, "phi.gguf");
        assert_eq!(limits.max_context, Some(131072));
        assert_eq!(limits.current_context, Some(21760));
    }

    #[test]
    fn a_model_known_by_file_name_matches_the_full_path_the_server_reports() {
        let entries = vec![entry("D:\\models\\phi.gguf", Some(4096), Some(2048))];
        assert_eq!(select_limits(&entries, "phi.gguf").max_context, Some(4096));
    }

    #[test]
    fn a_server_without_meta_reports_no_limits_instead_of_inventing_them() {
        let entries = vec![ModelEntry {
            id: "phi.gguf".to_string(),
            meta: None,
        }];
        let limits = select_limits(&entries, "phi.gguf");
        assert_eq!(limits.max_context, None);
        assert_eq!(limits.current_context, None);
    }

    #[tokio::test]
    async fn a_stopped_runtime_is_unavailable_rather_than_an_error_string() {
        let client = LlamaServerClient::new(None, PathBuf::from("/models"));
        assert!(matches!(
            client.model_limits("phi.gguf").await,
            Err(ProviderError::Unavailable)
        ));
        // Listing models still works with no server: they are files on disk.
        assert!(client.list_installed_models().is_empty());
    }
}
