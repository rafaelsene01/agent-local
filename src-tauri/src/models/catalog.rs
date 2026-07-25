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
    },
    CuratedModel {
        id: "llama3.2-3b",
        display_name: "Llama 3.2 3B",
        provider: "ollama",
        pull_identifier: "llama3.2:3b",
        params_billions: 3.21,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
    },
    CuratedModel {
        id: "qwen2.5-7b",
        display_name: "Qwen2.5 7B",
        provider: "ollama",
        pull_identifier: "qwen2.5:7b",
        params_billions: 7.62,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
    },
    CuratedModel {
        id: "qwen2.5-14b",
        display_name: "Qwen2.5 14B",
        provider: "ollama",
        pull_identifier: "qwen2.5:14b",
        params_billions: 14.77,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
    },
    CuratedModel {
        id: "phi3-mini",
        display_name: "Phi-3 Mini",
        provider: "ollama",
        pull_identifier: "phi3:mini",
        params_billions: 3.8,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
    },
    CuratedModel {
        id: "mistral-7b",
        display_name: "Mistral 7B",
        provider: "ollama",
        pull_identifier: "mistral:7b",
        params_billions: 7.25,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
    },
    CuratedModel {
        id: "gemma2-9b",
        display_name: "Gemma 2 9B",
        provider: "ollama",
        pull_identifier: "gemma2:9b",
        params_billions: 9.24,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
    },
    CuratedModel {
        id: "deepseek-r1-7b",
        display_name: "DeepSeek-R1 7B (distill)",
        provider: "ollama",
        pull_identifier: "deepseek-r1:7b",
        params_billions: 7.0,
        default_quant: "Q4_K_M",
        quant: Quant::Q4,
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

    #[test]
    fn every_curated_model_has_positive_estimated_ram() {
        for m in curated_models() {
            let info = CuratedModelInfo::from(m);
            assert!(info.estimated_ram_gb > 0.0, "{} has non-positive RAM estimate", info.id);
        }
    }
}
