use super::{ChatMessage, ChatStream, ChatToken, ProviderError};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: Delta,
}

#[derive(Debug, Deserialize)]
struct Delta {
    content: Option<String>,
}

/// LM Studio, llama.cpp's server and any OpenAI-compatible endpoint speak the
/// same SSE dialect, so the parser lives once here instead of three times.
/// Frames are `data: {json}` separated by blank lines, ending with
/// `data: [DONE]`.
pub async fn stream_chat_completions(
    client: &reqwest::Client,
    base_url: &str,
    model: &str,
    messages: Vec<ChatMessage>,
    max_context: Option<u32>,
) -> Result<ChatStream, ProviderError> {
    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
    });
    if let Some(ctx) = max_context {
        // Not a context-window setting per se: it caps the answer so a long
        // reply can't run past the window the user configured.
        body["max_tokens"] = json!(ctx);
    }

    let response = client
        .post(format!("{base_url}/v1/chat/completions"))
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(ProviderError::RequestFailed(format!("{status}: {detail}")));
    }

    let mut buffer = String::new();
    let stream = response.bytes_stream().flat_map(move |chunk| {
        let mut tokens: Vec<Result<ChatToken, ProviderError>> = Vec::new();
        match chunk {
            Ok(bytes) => {
                buffer.push_str(&String::from_utf8_lossy(&bytes));
                // A frame can be split across TCP reads, so only whole lines
                // are parsed and the tail stays buffered.
                while let Some(newline) = buffer.find('\n') {
                    let line = buffer[..newline].trim().to_string();
                    buffer.drain(..=newline);

                    let Some(payload) = line.strip_prefix("data:") else {
                        continue;
                    };
                    let payload = payload.trim();
                    if payload == "[DONE]" {
                        tokens.push(Ok(ChatToken {
                            delta: String::new(),
                            done: true,
                        }));
                        continue;
                    }
                    match serde_json::from_str::<StreamChunk>(payload) {
                        Ok(parsed) => {
                            if let Some(delta) = parsed
                                .choices
                                .first()
                                .and_then(|c| c.delta.content.clone())
                                .filter(|d| !d.is_empty())
                            {
                                tokens.push(Ok(ChatToken { delta, done: false }));
                            }
                        }
                        Err(e) => tokens.push(Err(ProviderError::ParseError(e.to_string()))),
                    }
                }
            }
            Err(e) => tokens.push(Err(ProviderError::from(e))),
        }
        futures_util::stream::iter(tokens)
    });

    Ok(Box::pin(stream))
}
