use super::{
    custom::CustomClient, ChatMessage, ChatStream, ConfigApplied, GpuOffload, InstalledModel,
    ModelLimits, ProviderClient, ProviderError, PullProgress,
};
use crate::runtime::model;
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::sync::mpsc::Sender;

/// The sidecar *is* an OpenAI-compatible server, so everything HTTP is the
/// `CustomClient` verbatim. What's different lives before that: there may be
/// no process listening at all, and downloads are plain GGUF files.
pub struct EmbeddedClient {
    port: Option<u16>,
    models_dir: PathBuf,
}

impl EmbeddedClient {
    pub fn new(port: Option<u16>, models_dir: PathBuf) -> Self {
        EmbeddedClient { port, models_dir }
    }

    /// `Unavailable` rather than an error string: "the sidecar isn't running"
    /// is exactly the same situation as "Ollama isn't running", and the UI
    /// already knows how to show it (EMBED-08).
    fn client(&self) -> Result<CustomClient, ProviderError> {
        let port = self.port.ok_or(ProviderError::Unavailable)?;
        Ok(CustomClient::new(format!("http://127.0.0.1:{port}")))
    }

    /// Installed = the GGUF files sitting in the models folder, not the single
    /// one the running server happens to have loaded. `/v1/models` would only
    /// ever report that one, and it carries no size — reading the directory
    /// answers both "what can I switch to" and "how big is it".
    fn installed_from_disk(&self) -> Vec<InstalledModel> {
        let Ok(entries) = std::fs::read_dir(&self.models_dir) else {
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
}

#[async_trait]
impl ProviderClient for EmbeddedClient {
    async fn health_check(&self) -> Result<(), ProviderError> {
        self.client()?.health_check().await
    }

    async fn list_installed_models(&self) -> Result<Vec<InstalledModel>, ProviderError> {
        Ok(self.installed_from_disk())
    }

    /// Only the running sidecar knows the model's trained window: it comes
    /// from the GGUF header it loaded, so a stopped runtime reports nothing.
    async fn model_limits(&self, model: &str) -> Result<ModelLimits, ProviderError> {
        self.client()?.model_limits(model).await
    }

    /// `identifier` is a direct `.gguf` URL (EMBED-13) — there is no registry
    /// to pull by name from, unlike Ollama.
    async fn stream_chat(
        &self,
        model: &str,
        messages: Vec<ChatMessage>,
        context_length: Option<u32>,
        gpu_offload: Option<GpuOffload>,
    ) -> Result<ChatStream, ProviderError> {
        self.client()?
            .stream_chat(model, messages, context_length, gpu_offload)
            .await
    }

    async fn pull_model(
        &self,
        identifier: &str,
        progress: Sender<PullProgress>,
    ) -> Result<(), ProviderError> {
        model::download_model_from_url(identifier, &self.models_dir, progress)
            .await
            .map(|_| ())
            .map_err(|e| ProviderError::RequestFailed(e.to_string()))
    }

    /// `--ctx-size` and `-ngl` are startup flags, so the caller persists them
    /// and restarts the process; `requires_reload` stays true because that is
    /// what actually happened — same honesty as LM Studio (M3).
    async fn configure_model(
        &self,
        _model: &str,
        context_length: Option<u32>,
        gpu_offload: Option<GpuOffload>,
    ) -> Result<ConfigApplied, ProviderError> {
        Ok(ConfigApplied {
            context_length_applied: context_length,
            gpu_offload_applied: gpu_offload.map(|g| g.to_value_string()),
            requires_reload: true,
            note: Some(
                "o runtime embutido aplica contexto e GPU ao iniciar — o servidor foi reiniciado com as novas configurações"
                    .to_string(),
            ),
        })
    }
}

/// `-ngl` takes a layer count, and the number of layers in a GGUF isn't known
/// without reading the model, so the embedded runtime supports all-or-nothing
/// offload only: a fraction can't be honored honestly and is treated as "off"
/// rather than silently becoming "max".
pub fn gpu_layers_for(offload: &GpuOffload) -> i32 {
    match offload {
        GpuOffload::Max => -1,
        GpuOffload::Off => 0,
        GpuOffload::Fraction(f) if *f >= 1.0 => -1,
        GpuOffload::Fraction(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_models_are_the_gguf_files_with_their_sizes() {
        let dir = std::env::temp_dir().join(format!("localmind-models-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Phi-3.5-mini-instruct-Q4_K_M.gguf"), vec![0u8; 2048]).unwrap();
        std::fs::write(dir.join("Qwen2.5-1.5B-Instruct-Q4_K_M.GGUF"), vec![0u8; 4096]).unwrap();
        // The embedding model cache shares this folder — it must not show up.
        std::fs::write(dir.join("model.onnx"), vec![0u8; 10]).unwrap();

        let models = EmbeddedClient::new(None, dir.clone()).installed_from_disk();

        assert_eq!(models.len(), 2, "only .gguf files are models");
        assert_eq!(models[0].name, "Phi-3.5-mini-instruct-Q4_K_M.gguf");
        assert_eq!(models[0].size_bytes, Some(2048));
        assert_eq!(models[1].size_bytes, Some(4096), "uppercase .GGUF counts too");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_models_folder_lists_nothing_instead_of_failing() {
        let dir = std::env::temp_dir().join("localmind-models-does-not-exist");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(EmbeddedClient::new(None, dir).installed_from_disk().is_empty());
    }

    #[test]
    fn gpu_offload_maps_to_all_or_nothing_layers() {
        assert_eq!(gpu_layers_for(&GpuOffload::Max), -1);
        assert_eq!(gpu_layers_for(&GpuOffload::Off), 0);
        assert_eq!(gpu_layers_for(&GpuOffload::Fraction(1.0)), -1);
        assert_eq!(gpu_layers_for(&GpuOffload::Fraction(0.5)), 0);
    }
}
