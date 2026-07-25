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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    /// Serves one canned SSE response, deliberately flushing mid-frame so the
    /// parser has to survive a JSON object split across two TCP reads — the
    /// failure mode that only shows up against a real socket.
    fn spawn_fake_server(body_parts: Vec<&'static str>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = [0u8; 2048];
            use std::io::Read;
            let _ = socket.read(&mut request);
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
            for part in body_parts {
                socket.write_all(part.as_bytes()).unwrap();
                socket.flush().unwrap();
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        });
        format!("http://127.0.0.1:{port}")
    }

    #[tokio::test]
    async fn parses_deltas_across_split_frames_and_stops_on_done() {
        let base = spawn_fake_server(vec![
            "data: {\"choices\":[{\"delta\":{\"content\":\"Olá\"}}]}\n\n",
            // This frame arrives in two pieces.
            "data: {\"choices\":[{\"delta\":{\"cont",
            "ent\":\", mundo\"}}]}\n\ndata: [DONE]\n\n",
        ]);

        let client = reqwest::Client::new();
        let mut stream = stream_chat_completions(
            &client,
            &base,
            "any-model",
            vec![ChatMessage::user("oi")],
            None,
        )
        .await
        .expect("request failed");

        let mut text = String::new();
        let mut saw_done = false;
        while let Some(item) = stream.next().await {
            let token = item.expect("stream item failed");
            text.push_str(&token.delta);
            if token.done {
                saw_done = true;
                break;
            }
        }

        assert_eq!(text, "Olá, mundo");
        assert!(saw_done, "[DONE] must surface as a done token");
    }

    #[tokio::test]
    async fn an_http_error_is_reported_instead_of_an_empty_stream() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            use std::io::Read;
            let _ = socket.read(&mut [0u8; 2048]);
            let _ = socket.write_all(
                b"HTTP/1.1 404 Not Found\r\nContent-Length: 13\r\nConnection: close\r\n\r\nmodel missing",
            );
        });

        let client = reqwest::Client::new();
        let result = stream_chat_completions(
            &client,
            &format!("http://127.0.0.1:{port}"),
            "missing",
            vec![ChatMessage::user("oi")],
            None,
        )
        .await;

        match result {
            Err(ProviderError::RequestFailed(msg)) => assert!(msg.contains("404")),
            Err(other) => panic!("expected a RequestFailed error, got {other}"),
            Ok(_) => panic!("a 404 must not produce a stream"),
        }
    }
}
