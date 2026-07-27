pub mod llama_server;
pub mod openai_stream;

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Every provider lives on localhost, so an unreachable one should fail fast
/// instead of holding the Conexões screen. Applied per-request rather than on
/// the client, which also serves streaming answers and model downloads.
pub const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(2);

/// Hard ceiling on a single answer. Without one, generation is unlimited and
/// a model that misses its stop token keeps going until the whole context
/// window is full — seen live: 6000+ tokens of runaway text after a malformed
/// prompt. Long answers are truncated visibly, which beats a hung chat.
pub const MAX_ANSWER_TOKENS: u32 = 2048;

/// The answer also has to leave room for the prompt inside the configured
/// window, so a small window shrinks the cap instead of overflowing it.
pub fn answer_token_budget(context_length: Option<u32>) -> u32 {
    match context_length {
        Some(ctx) => MAX_ANSWER_TOKENS.min((ctx / 2).max(256)),
        None => MAX_ANSWER_TOKENS,
    }
}

/// For calls that must answer promptly (listing models, applying config) —
/// generous enough for a runtime that is busy loading a model.
pub const SHORT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Shared by every provider client. There is deliberately **no** overall
/// timeout: `reqwest`'s applies to the whole request including the response
/// body, so a total timeout also caps how long an answer may stream and how
/// long a model pull may take — a 5s one killed generation mid-sentence
/// (`llama-server` logged `stop: cancel task` five seconds in). Connecting
/// still fails fast, and short calls set their own per-request timeout.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("failed to build reqwest client")
}

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

/// How much of the model to put on the GPU. Persisted in
/// `embedded_runtime.gpu_layers` after being reduced to a layer count.
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

/// What the provider says about a model's context window. `max_context` is
/// the length the model was trained for — the ceiling for the config field —
/// and `current_context` is what the runtime has allocated right now, which
/// can be smaller (llama.cpp sizes the KV cache to fit memory).
///
/// Both are optional: a plain OpenAI-compatible server reports neither, and a
/// number invented for it would be a lie the UI would enforce.
#[derive(Debug, Serialize, Clone, Default)]
pub struct ModelLimits {
    pub max_context: Option<u32>,
    pub current_context: Option<u32>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        ChatMessage {
            role: "system".to_string(),
            content: content.into(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        ChatMessage {
            role: "user".to_string(),
            content: content.into(),
        }
    }
}

/// One incremental piece of the answer. `done` arrives on its own at the end,
/// which is what lets the caller persist the accumulated message exactly once.
#[derive(Debug, Clone)]
pub struct ChatToken {
    pub delta: String,
    pub done: bool,
}

pub type ChatStream = std::pin::Pin<
    Box<dyn futures_util::Stream<Item = Result<ChatToken, ProviderError>> + Send>,
>;

/// `-ngl` takes a layer count, and the number of layers in a GGUF is not known
/// without reading the model, so offload is all-or-nothing: a fraction cannot
/// be honored honestly and is treated as off rather than silently becoming
/// max.
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
    fn an_answer_is_always_capped_and_never_exceeds_half_the_window() {
        assert_eq!(answer_token_budget(None), MAX_ANSWER_TOKENS);
        assert_eq!(answer_token_budget(Some(131072)), MAX_ANSWER_TOKENS);
        assert_eq!(answer_token_budget(Some(2048)), 1024);
        // A tiny window still leaves room for a usable answer.
        assert_eq!(answer_token_budget(Some(512)), 256);
    }

    #[test]
    fn gpu_offload_maps_to_all_or_nothing_layers() {
        assert_eq!(gpu_layers_for(&GpuOffload::Max), -1);
        assert_eq!(gpu_layers_for(&GpuOffload::Off), 0);
        assert_eq!(gpu_layers_for(&GpuOffload::Fraction(1.0)), -1);
        assert_eq!(gpu_layers_for(&GpuOffload::Fraction(0.5)), 0);
    }

    #[test]
    fn gpu_offload_round_trips_through_its_persisted_string() {
        for value in [GpuOffload::Off, GpuOffload::Max, GpuOffload::Fraction(0.5)] {
            let text = value.to_value_string();
            assert_eq!(GpuOffload::parse(&text).unwrap(), value);
        }
        assert!(GpuOffload::parse("metade").is_err());
    }
}
