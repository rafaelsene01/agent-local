use super::{download, RuntimeError};
use crate::providers::PullProgress;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::Sender;

/// Phi-3.5 Mini Instruct, Q4_K_M — MIT licensed, the user's explicit choice
/// over two smaller Apache-2.0 alternatives (context.md).
///
/// Verified live on 2026-07-25: `HEAD` on this URL follows one redirect and
/// answers 200 with `content-length: 2393232672` (~2.39 GB), matching the
/// size the design expected. Do not change it without re-checking.
pub const DEFAULT_MODEL_URL: &str =
    "https://huggingface.co/bartowski/Phi-3.5-mini-instruct-GGUF/resolve/main/Phi-3.5-mini-instruct-Q4_K_M.gguf";

pub const DEFAULT_MODEL_FILE: &str = "Phi-3.5-mini-instruct-Q4_K_M.gguf";

/// Roughly 2.4 GB — shown to the user before the download starts so the size
/// is never a surprise (EMBED-03).
pub const DEFAULT_MODEL_SIZE_BYTES: u64 = 2_393_232_672;

pub fn default_model_path(models_dir: &Path) -> PathBuf {
    models_dir.join(DEFAULT_MODEL_FILE)
}

pub async fn download_default_model(
    models_dir: &Path,
    progress: Sender<PullProgress>,
) -> Result<PathBuf, RuntimeError> {
    let dest = default_model_path(models_dir);
    if dest.exists() {
        return Ok(dest);
    }
    download::download_with_progress(DEFAULT_MODEL_URL, &dest, progress).await?;
    Ok(dest)
}

/// The sidecar can only load GGUF files, so a link to a repo page or a
/// safetensors file is rejected up front instead of downloading gigabytes
/// that `llama-server` would then refuse (EMBED-13).
pub fn validate_gguf_url(url: &str) -> Result<String, RuntimeError> {
    let without_query = url.split(['?', '#']).next().unwrap_or(url);
    if !without_query.to_lowercase().ends_with(".gguf") {
        return Err(RuntimeError::Io(
            "o link precisa apontar direto para um arquivo .gguf".to_string(),
        ));
    }
    let file_name = without_query
        .rsplit('/')
        .next()
        .filter(|n| !n.is_empty())
        .ok_or_else(|| RuntimeError::Io("não foi possível extrair o nome do arquivo do link".to_string()))?;
    Ok(file_name.to_string())
}

pub async fn download_model_from_url(
    url: &str,
    models_dir: &Path,
    progress: Sender<PullProgress>,
) -> Result<PathBuf, RuntimeError> {
    let file_name = validate_gguf_url(url)?;
    let dest = models_dir.join(file_name);
    // Already downloaded is a success, not a reason to pull gigabytes again.
    if dest.exists() {
        return Ok(dest);
    }
    download::download_with_progress(url, &dest, progress).await?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_direct_gguf_link_and_keeps_the_file_name() {
        let name = validate_gguf_url(DEFAULT_MODEL_URL).unwrap();
        assert_eq!(name, DEFAULT_MODEL_FILE);

        let with_query =
            validate_gguf_url("https://example.com/models/tiny-model.GGUF?download=true").unwrap();
        assert_eq!(with_query, "tiny-model.GGUF");
    }

    #[test]
    fn rejects_anything_that_is_not_a_gguf_file() {
        for url in [
            "https://huggingface.co/bartowski/Phi-3.5-mini-instruct-GGUF",
            "https://example.com/model.safetensors",
            "https://example.com/model.gguf.zip",
        ] {
            assert!(validate_gguf_url(url).is_err(), "should reject {url}");
        }
    }
}
