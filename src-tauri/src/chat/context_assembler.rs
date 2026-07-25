use crate::db::DbState;
use crate::providers::ChatMessage;
use crate::rag::store::{chat_namespace, VectorStore, GLOBAL_NAMESPACE};
use crate::rag::{embedding, onnxruntime, pipeline};
use rusqlite::params;
use tauri::{AppHandle, Manager};

/// Rough characters-per-token used for budgeting. Exact tokenization varies
/// per model; being slightly conservative here costs a little context and
/// avoids overflowing the window, which would fail the whole request.
const CHARS_PER_TOKEN: usize = 4;

/// Used when no context length was configured for the active model.
const DEFAULT_CONTEXT_TOKENS: u32 = 4096;

/// Room left for the model's own answer.
const RESPONSE_RESERVE_TOKENS: u32 = 512;

const TOP_K: usize = 4;
const RECENT_HISTORY_LIMIT: usize = 20;

const SYSTEM_PROMPT: &str = "Você é um assistente local e privado. Responda de forma direta. \
Quando trechos de documentos forem fornecidos como contexto, baseie a resposta neles e diga \
claramente quando a informação não estiver no contexto.";

fn budget_chars(context_length: Option<u32>) -> usize {
    let total = context_length.unwrap_or(DEFAULT_CONTEXT_TOKENS);
    let usable = total.saturating_sub(RESPONSE_RESERVE_TOKENS);
    usable as usize * CHARS_PER_TOKEN
}

struct Budget {
    remaining: usize,
}

impl Budget {
    /// Takes what fits and truncates the rest, instead of dropping a whole
    /// category — a partially included document still answers questions
    /// (CHAT-15).
    fn take(&mut self, text: &str) -> Option<String> {
        if self.remaining == 0 {
            return None;
        }
        if text.len() <= self.remaining {
            self.remaining -= text.len();
            return Some(text.to_string());
        }
        let mut cut = self.remaining;
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        self.remaining = 0;
        if cut == 0 {
            None
        } else {
            Some(text[..cut].to_string())
        }
    }
}

fn recent_history(app: &AppHandle, chat_id: &str) -> Result<Vec<ChatMessage>, String> {
    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = crate::db::require_conn(&guard)?;
    let mut stmt = sql
        .prepare(
            "SELECT role, content FROM messages WHERE chat_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let mut rows: Vec<ChatMessage> = stmt
        .query_map(params![chat_id, RECENT_HISTORY_LIMIT as i64], |row| {
            Ok(ChatMessage {
                role: row.get(0)?,
                content: row.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    // Queried newest-first for the LIMIT, replayed oldest-first for the model.
    rows.reverse();
    Ok(rows)
}

/// Whole small attachments bypass retrieval entirely (CHAT-09): chunking a
/// three-line note only loses information.
fn injected_attachments(app: &AppHandle, chat_id: &str) -> Result<Vec<(String, String)>, String> {
    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = crate::db::require_conn(&guard)?;
    let mut stmt = sql
        .prepare(
            "SELECT filename, extracted_text FROM chat_attachments
             WHERE chat_id = ?1 AND status = 'injected_whole' AND extracted_text IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![chat_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Builds the final message list: system prompt, then context in priority
/// order (chat attachments, recent history, global documents), then the
/// question itself. The question is never truncated — everything else is.
pub async fn assemble(
    app: &AppHandle,
    chat_id: &str,
    new_message: &str,
    use_global_rag: bool,
    context_length: Option<u32>,
) -> Result<Vec<ChatMessage>, String> {
    let mut budget = Budget {
        remaining: budget_chars(context_length).saturating_sub(new_message.len()),
    };

    let mut context_blocks: Vec<String> = Vec::new();

    for (filename, text) in injected_attachments(app, chat_id)? {
        if let Some(part) = budget.take(&format!("[anexo: {filename}]\n{text}")) {
            context_blocks.push(part);
        }
    }

    // Retrieval only runs when something was indexed; embedding the question
    // otherwise would download a model for nothing.
    let chat_ns = chat_namespace(chat_id);
    let namespaces: Vec<&str> = if use_global_rag {
        vec![chat_ns.as_str(), GLOBAL_NAMESPACE]
    } else {
        vec![chat_ns.as_str()]
    };

    if budget.remaining > 0 {
        match retrieve(app, &namespaces, new_message).await {
            Ok(chunks) => {
                for chunk in chunks {
                    if let Some(part) = budget.take(&chunk) {
                        context_blocks.push(part);
                    }
                }
            }
            // Retrieval is an enhancement: a broken vector store or a missing
            // embedding model must not block the conversation.
            Err(e) => eprintln!("retrieval skipped: {e}"),
        }
    }

    let mut messages = vec![ChatMessage::system(SYSTEM_PROMPT)];
    if !context_blocks.is_empty() {
        messages.push(ChatMessage::system(format!(
            "Contexto recuperado:\n\n{}",
            context_blocks.join("\n\n---\n\n")
        )));
    }

    for message in recent_history(app, chat_id)? {
        if let Some(content) = budget.take(&message.content) {
            messages.push(ChatMessage {
                role: message.role,
                content,
            });
        }
    }

    messages.push(ChatMessage::user(new_message));
    Ok(messages)
}

async fn retrieve(
    app: &AppHandle,
    namespaces: &[&str],
    question: &str,
) -> Result<Vec<String>, String> {
    let store = VectorStore::open(&pipeline::vectors_dir(app)?)
        .await
        .map_err(|e| e.to_string())?;

    onnxruntime::ensure_dylib(app).await?;
    let question = question.to_string();
    let query_vec = tauri::async_runtime::spawn_blocking(move || embedding::embed_query(&question))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    let mut chunks = Vec::new();
    for namespace in namespaces {
        let found = store
            .search(namespace, &query_vec, TOP_K)
            .await
            .map_err(|e| e.to_string())?;
        chunks.extend(found.into_iter().map(|c| c.text));
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_truncates_instead_of_dropping() {
        let mut budget = Budget { remaining: 10 };
        assert_eq!(budget.take("12345").unwrap(), "12345");
        assert_eq!(budget.take("abcdefghij").unwrap(), "abcde");
        assert!(budget.take("more").is_none());
    }

    #[test]
    fn truncation_never_splits_a_multibyte_character() {
        let mut budget = Budget { remaining: 3 };
        // 'ç' is two bytes, so cutting at byte 3 would land mid-character.
        let taken = budget.take("açaí").unwrap();
        assert!(taken.chars().all(|c| "açaí".contains(c)));
    }

    #[test]
    fn context_budget_leaves_room_for_the_answer() {
        let chars = budget_chars(Some(1024));
        assert_eq!(chars, (1024 - 512) * CHARS_PER_TOKEN);
        assert!(budget_chars(Some(100)) == 0, "a tiny window reserves nothing");
    }
}
