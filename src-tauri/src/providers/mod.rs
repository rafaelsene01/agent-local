pub mod custom;
pub mod embedded;
pub mod lmstudio;
pub mod ollama;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::mpsc::Sender;

/// Every provider lives on localhost, so an unreachable one should fail fast
/// instead of holding the Conexões screen for the client-wide 5s. Applied
/// per-request rather than on the client, which also serves model downloads.
pub const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Serialize, Clone)]
pub struct InstalledModel {
    pub name: String,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PullStatus {
    Downloading,
    Verifying,
    Success,
    Error,
}

#[derive(Debug, Serialize, Clone)]
pub struct PullProgress {
    pub status: PullStatus,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub message: Option<String>,
}

/// Mirrors the string values persisted in `model_configs.gpu_offload`
/// ('off' | 'max' | a fraction like '0.5').
#[derive(Debug, Clone, PartialEq)]
pub enum GpuOffload {
    Off,
    Max,
    Fraction(f32),
}

impl GpuOffload {
    pub fn to_value_string(&self) -> String {
        match self {
            GpuOffload::Off => "off".to_string(),
            GpuOffload::Max => "max".to_string(),
            GpuOffload::Fraction(f) => f.to_string(),
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "off" => Ok(GpuOffload::Off),
            "max" => Ok(GpuOffload::Max),
            other => other
                .parse::<f32>()
                .map(GpuOffload::Fraction)
                .map_err(|_| format!("invalid gpu_offload value: {other}")),
        }
    }
}

impl Serialize for GpuOffload {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_value_string())
    }
}

impl<'de> Deserialize<'de> for GpuOffload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        GpuOffload::parse(&s).map_err(serde::de::Error::custom)
    }
}

/// Reports which fields the provider actually accepted (CONN-13 AC3) —
/// e.g. LM Studio only applies context/GPU config on model (re)load,
/// Ollama applies num_ctx/num_gpu per chat request without a reload.
#[derive(Debug, Serialize, Clone)]
pub struct ConfigApplied {
    pub context_length_applied: Option<u32>,
    pub gpu_offload_applied: Option<String>,
    pub requires_reload: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ProviderError {
    Unavailable,
    RequestFailed(String),
    ParseError(String),
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Unavailable => write!(f, "provider unavailable"),
            ProviderError::RequestFailed(msg) => write!(f, "request failed: {msg}"),
            ProviderError::ParseError(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for ProviderError {}

impl From<reqwest::Error> for ProviderError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_connect() || err.is_timeout() {
            ProviderError::Unavailable
        } else {
            ProviderError::RequestFailed(err.to_string())
        }
    }
}

#[async_trait]
pub trait ProviderClient: Send + Sync {
    async fn health_check(&self) -> Result<(), ProviderError>;
    async fn list_installed_models(&self) -> Result<Vec<InstalledModel>, ProviderError>;
    async fn pull_model(
        &self,
        identifier: &str,
        progress: Sender<PullProgress>,
    ) -> Result<(), ProviderError>;
    async fn configure_model(
        &self,
        model: &str,
        context_length: Option<u32>,
        gpu_offload: Option<GpuOffload>,
    ) -> Result<ConfigApplied, ProviderError>;
}
