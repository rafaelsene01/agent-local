use super::{
    custom::CustomClient, ConfigApplied, GpuOffload, InstalledModel, ProviderClient, ProviderError,
    PullProgress,
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
}

#[async_trait]
impl ProviderClient for EmbeddedClient {
    async fn health_check(&self) -> Result<(), ProviderError> {
        self.client()?.health_check().await
    }

    async fn list_installed_models(&self) -> Result<Vec<InstalledModel>, ProviderError> {
        self.client()?.list_installed_models().await
    }

    /// `identifier` is a direct `.gguf` URL (EMBED-13) — there is no registry
    /// to pull by name from, unlike Ollama.
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
    fn gpu_offload_maps_to_all_or_nothing_layers() {
        assert_eq!(gpu_layers_for(&GpuOffload::Max), -1);
        assert_eq!(gpu_layers_for(&GpuOffload::Off), 0);
        assert_eq!(gpu_layers_for(&GpuOffload::Fraction(1.0)), -1);
        assert_eq!(gpu_layers_for(&GpuOffload::Fraction(0.5)), 0);
    }
}
