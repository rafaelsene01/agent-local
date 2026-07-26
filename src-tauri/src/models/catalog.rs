use super::memory_estimate::{estimate_ram_gb, Quant};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct CuratedModel {
    pub id: &'static str,
    pub display_name: &'static str,
    pub provider: &'static str,
    pub pull_identifier: &'static str,
    pub params_billions: f32,
    pub default_quant: &'static str,
    quant: Quant,
    /// Exact download size, known only for the embedded runtime's GGUF files
    /// (each URL was checked and its `content-length` recorded). Ollama pulls
    /// have no size until the pull starts, so they carry `None` and fall back
    /// to the RAM estimate.
    download_bytes: Option<u64>,
}

/// Serializable projection sent to the frontend — includes the computed
/// `estimated_ram_gb`, which isn't stored on `CuratedModel` itself.
#[derive(Debug, Serialize, Clone)]
pub struct CuratedModelInfo {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub pull_identifier: String,
    pub params_billions: f32,
    pub default_quant: String,
    pub estimated_ram_gb: f32,
    pub download_bytes: Option<u64>,
}

impl From<&CuratedModel> for CuratedModelInfo {
    fn from(m: &CuratedModel) -> Self {
        CuratedModelInfo {
            id: m.id.to_string(),
            display_name: m.display_name.to_string(),
            provider: m.provider.to_string(),
            pull_identifier: m.pull_identifier.to_string(),
            params_billions: m.params_billions,
            default_quant: m.default_quant.to_string(),
            estimated_ram_gb: estimate_ram_gb(m.params_billions, m.quant),
            download_bytes: m.download_bytes,
        }
    }
}

const CURATED_MODELS: &[CuratedModel] = &[
    CuratedModel {
        id: "llama3.1-8b",
        display_name: "Llama 3.1 8B",
        provider: "ollama",
        pull_identifier: "llama3.1:8b",
        params_billions: 8.03,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: None,
    },
    CuratedModel {
        id: "llama3.2-3b",
        display_name: "Llama 3.2 3B",
        provider: "ollama",
        pull_identifier: "llama3.2:3b",
        params_billions: 3.21,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: None,
    },
    CuratedModel {
        id: "qwen2.5-7b",
        display_name: "Qwen2.5 7B",
        provider: "ollama",
        pull_identifier: "qwen2.5:7b",
        params_billions: 7.62,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: None,
    },
    CuratedModel {
        id: "qwen2.5-14b",
        display_name: "Qwen2.5 14B",
        provider: "ollama",
        pull_identifier: "qwen2.5:14b",
        params_billions: 14.77,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: None,
    },
    CuratedModel {
        id: "phi3-mini",
        display_name: "Phi-3 Mini",
        provider: "ollama",
        pull_identifier: "phi3:mini",
        params_billions: 3.8,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: None,
    },
    CuratedModel {
        id: "mistral-7b",
        display_name: "Mistral 7B",
        provider: "ollama",
        pull_identifier: "mistral:7b",
        params_billions: 7.25,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: None,
    },
    CuratedModel {
        id: "gemma2-9b",
        display_name: "Gemma 2 9B",
        provider: "ollama",
        pull_identifier: "gemma2:9b",
        params_billions: 9.24,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: None,
    },
    CuratedModel {
        id: "deepseek-r1-7b",
        display_name: "DeepSeek-R1 7B (distill)",
        provider: "ollama",
        pull_identifier: "deepseek-r1:7b",
        params_billions: 7.0,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: None,
    },
    // Embedded runtime: `pull_identifier` is the direct `.gguf` URL the
    // sidecar downloads (EMBED-13) — there is no registry to pull by name.
    // Every URL below was checked with `HEAD` on 2026-07-25 and answered 200;
    // `download_bytes` is the `content-length` that came back. Re-check before
    // changing any of them.
    CuratedModel {
        id: "gguf-qwen2.5-1.5b",
        display_name: "Qwen2.5 1.5B Instruct",
        provider: "embedded",
        pull_identifier: "https://huggingface.co/bartowski/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/Qwen2.5-1.5B-Instruct-Q4_K_M.gguf",
        params_billions: 1.54,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: Some(986_048_768),
    },
    CuratedModel {
        id: "gguf-llama3.2-3b",
        display_name: "Llama 3.2 3B Instruct",
        provider: "embedded",
        pull_identifier: "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
        params_billions: 3.21,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: Some(2_019_377_696),
    },
    CuratedModel {
        id: "gguf-phi3.5-mini",
        display_name: "Phi-3.5 Mini Instruct",
        provider: "embedded",
        pull_identifier: "https://huggingface.co/bartowski/Phi-3.5-mini-instruct-GGUF/resolve/main/Phi-3.5-mini-instruct-Q4_K_M.gguf",
        params_billions: 3.8,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: Some(2_393_232_672),
    },
    CuratedModel {
        id: "gguf-mistral-7b",
        display_name: "Mistral 7B Instruct v0.3",
        provider: "embedded",
        pull_identifier: "https://huggingface.co/bartowski/Mistral-7B-Instruct-v0.3-GGUF/resolve/main/Mistral-7B-Instruct-v0.3-Q4_K_M.gguf",
        params_billions: 7.25,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: Some(4_372_812_000),
    },
    CuratedModel {
        id: "gguf-qwen2.5-7b",
        display_name: "Qwen2.5 7B Instruct",
        provider: "embedded",
        pull_identifier: "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf",
        params_billions: 7.62,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: Some(4_683_074_240),
    },
    CuratedModel {
        id: "gguf-llama3.1-8b",
        display_name: "Llama 3.1 8B Instruct",
        provider: "embedded",
        pull_identifier: "https://huggingface.co/bartowski/Meta-Llama-3.1-8B-Instruct-GGUF/resolve/main/Meta-Llama-3.1-8B-Instruct-Q4_K_M.gguf",
        params_billions: 8.03,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
        download_bytes: Some(4_920_739_232),
    },
];

pub fn curated_models() -> &'static [CuratedModel] {
    CURATED_MODELS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curated_models_has_at_least_six_entries() {
        assert!(curated_models().len() >= 6);
    }

    /// The embedded runtime downloads files, not registry names: a wrong
    /// identifier here means a multi-GB download the sidecar can't load.
    #[test]
    fn embedded_entries_point_at_a_direct_gguf_and_declare_their_size() {
        let embedded: Vec<_> = curated_models()
            .iter()
            .filter(|m| m.provider == "embedded")
            .collect();
        assert!(embedded.len() >= 4, "the embedded runtime needs real options");

        for m in embedded {
            assert!(
                crate::runtime::model::validate_gguf_url(m.pull_identifier).is_ok(),
                "{} is not a direct .gguf link",
                m.id
            );
            assert!(
                m.download_bytes.is_some_and(|b| b > 100_000_000),
                "{} must carry the size checked against the server",
                m.id
            );
        }
    }

    #[test]
    fn every_curated_model_has_positive_estimated_ram() {
        for m in curated_models() {
            let info = CuratedModelInfo::from(m);
            assert!(info.estimated_ram_gb > 0.0, "{} has non-positive RAM estimate", info.id);
        }
    }
}
