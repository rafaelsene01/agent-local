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

/// The "don't offer more help" clauses are aimed at Phi-3.5's habit of closing
/// every answer with a courtesy paragraph ("sinta-se à vontade para
/// perguntar"). It reduces the filler; it doesn't eliminate it, because the
/// tendency is the model's own.
const SYSTEM_PROMPT: &str = "Você é um assistente local e privado. Responda apenas o que foi \
perguntado, no menor número de frases possível. Não repita a pergunta, não ofereça ajuda \
adicional, não comente a própria resposta e não escreva parágrafos de cortesia. Diga claramente \
quando não souber.";

/// The citation instruction rides with the context, not with the base prompt:
/// a small model told to cite sources when none were given invents them —
/// observed live, answering with "[fonte: GPT-3 informações geral]".
const CONTEXT_PREAMBLE: &str = "Trechos dos documentos do usuário. Baseie a resposta neles e \
cite o nome do arquivo indicado em [fonte: ...] ao usar um trecho. Se a resposta não estiver \
aqui, diga isso.";

/// Every retrieved chunk carries the file it came from (DOC-12): without it
/// the model has nothing to cite and the user can't check the answer.
fn source_block(label: &str, text: &str) -> String {
    format!("[fonte: {label}]\n{text}")
}

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

/// `exclude_id` is the message being answered: `send_message` persists it
/// before assembling, so without this it would come back from the database and
/// be sent again right before itself. Two identical user turns in a row make
/// the model ramble instead of answering — verified against the sidecar, where
/// the duplicated prompt never produced a stop token and the single one did.
fn recent_history(
    app: &AppHandle,
    chat_id: &str,
    exclude_id: &str,
) -> Result<Vec<ChatMessage>, String> {
    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = crate::db::require_conn(&guard)?;
    let mut stmt = sql
        .prepare(
            "SELECT role, content FROM messages WHERE chat_id = ?1 AND id <> ?2
             ORDER BY created_at DESC LIMIT ?3",
        )
        .map_err(|e| e.to_string())?;
    let mut rows: Vec<ChatMessage> = stmt
        .query_map(params![chat_id, exclude_id, RECENT_HISTORY_LIMIT as i64], |row| {
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

/// Keeps the newest turns and drops the oldest when the budget runs out.
///
/// The naive direction — walking the history oldest-first and taking what fits
/// — spends the budget on turns the model no longer needs and starves the
/// exchange the question actually follows from.
fn fit_history(history: Vec<ChatMessage>, budget: &mut Budget) -> Vec<ChatMessage> {
    let mut kept: Vec<ChatMessage> = Vec::new();
    for message in history.into_iter().rev() {
        let Some(content) = budget.take(&message.content) else {
            break;
        };
        kept.push(ChatMessage {
            role: message.role,
            content,
        });
    }
    kept.reverse();
    kept
}

/// Puts the retrieved passages in the same turn as the question, right above
/// it, instead of in a system block at the top of the prompt.
///
/// A small model answers from what sits next to the question. With the document
/// thousands of tokens away — behind the whole history — Phi-3.5 reproduced its
/// own earlier answers verbatim instead of the source: three wrong
/// continuations of the same article, the last one a copy of the first.
fn question_with_context(new_message: &str, context_blocks: &[String]) -> String {
    if context_blocks.is_empty() {
        return new_message.to_string();
    }
    format!(
        "{CONTEXT_PREAMBLE}\n\n{}\n\n---\n\n{new_message}",
        context_blocks.join("\n\n---\n\n")
    )
}

/// Builds the final message list: system prompt, recent history, then the
/// question with the retrieved passages attached to it. The question is never
/// truncated — everything else is.
pub async fn assemble(
    app: &AppHandle,
    chat_id: &str,
    new_message: &str,
    new_message_id: &str,
    use_global_rag: bool,
    context_length: Option<u32>,
) -> Result<Vec<ChatMessage>, String> {
    let mut budget = Budget {
        remaining: budget_chars(context_length).saturating_sub(new_message.len()),
    };

    let mut context_blocks: Vec<String> = Vec::new();

    for (filename, text) in injected_attachments(app, chat_id)? {
        if let Some(part) = budget.take(&source_block(&filename, &text)) {
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
    let history = recent_history(app, chat_id, new_message_id)?;
    messages.extend(fit_history(history, &mut budget));
    messages.push(ChatMessage::user(question_with_context(
        new_message,
        &context_blocks,
    )));
    Ok(merge_consecutive_turns(messages))
}

/// Chat templates assume the roles alternate. A generation that was cancelled
/// or failed leaves the question persisted with no answer after it, so every
/// later request would carry two `user` turns in a row.
///
/// Measured against the running sidecar, that single malformation flips
/// Phi-3.5 from answering to acknowledging: "Entendido! Se você tiver mais
/// perguntas… fique à vontade" instead of "Olá! Como posso ajudá-lo hoje?".
/// Merging keeps every word the user wrote while restoring the alternation.
fn merge_consecutive_turns(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let mut merged: Vec<ChatMessage> = Vec::with_capacity(messages.len());
    for message in messages {
        match merged.last_mut() {
            Some(previous) if previous.role == message.role => {
                previous.content.push_str("\n\n");
                previous.content.push_str(&message.content);
            }
            _ => merged.push(message),
        }
    }
    merged
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
        for chunk in found {
            let label = source_name(app, namespace, &chunk.doc_id);
            chunks.push(source_block(&label, &chunk.text));
        }
    }
    Ok(chunks)
}

/// Resolves a `doc_id` back to the file the user recognizes. Global chunks
/// come from `documents`, chat chunks from `chat_attachments`; an id with no
/// row left (deleted mid-flight) falls back to a generic label instead of
/// failing the whole retrieval.
fn source_name(app: &AppHandle, namespace: &str, doc_id: &str) -> String {
    let fallback = || "documento".to_string();
    let db = app.state::<DbState>();
    let Ok(guard) = db.0.lock() else {
        return fallback();
    };
    let Some(sql) = guard.as_ref() else {
        return fallback();
    };
    let query = if namespace == GLOBAL_NAMESPACE {
        "SELECT filename FROM documents WHERE id = ?1"
    } else {
        "SELECT filename FROM chat_attachments WHERE id = ?1"
    };
    sql.query_row(query, params![doc_id], |row| row.get::<_, String>(0))
        .unwrap_or_else(|_| fallback())
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
    fn an_unanswered_question_does_not_leave_two_user_turns_in_a_row() {
        let merged = merge_consecutive_turns(vec![
            ChatMessage::system("regras"),
            ChatMessage::system("contexto"),
            // The generation for this one was cancelled, so no answer follows.
            ChatMessage::user("oi"),
            ChatMessage::user("o que é um banco de dados?"),
        ]);

        let roles: Vec<&str> = merged.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, vec!["system", "user"]);
        assert_eq!(merged[1].content, "oi\n\no que é um banco de dados?");
        assert!(merged[0].content.contains("regras") && merged[0].content.contains("contexto"));
    }

    #[test]
    fn a_healthy_conversation_is_left_exactly_as_it_is() {
        let original = vec![
            ChatMessage::system("regras"),
            ChatMessage::user("pergunta"),
            ChatMessage {
                role: "assistant".to_string(),
                content: "resposta".to_string(),
            },
            ChatMessage::user("outra pergunta"),
        ];

        let merged = merge_consecutive_turns(original.clone());

        assert_eq!(merged.len(), original.len());
        let roles: Vec<&str> = merged.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, vec!["system", "user", "assistant", "user"]);
    }

    fn turn(role: &str, content: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn a_tight_budget_drops_the_oldest_turns_and_keeps_the_newest() {
        let history = vec![
            turn("user", "pergunta antiga"),
            turn("assistant", "resposta antiga"),
            turn("user", "pergunta recente"),
        ];
        // Room for the last turn only.
        let mut budget = Budget { remaining: 16 };

        let kept = fit_history(history, &mut budget);

        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].content, "pergunta recente");
    }

    #[test]
    fn history_that_fits_stays_in_chronological_order() {
        let history = vec![
            turn("user", "primeira"),
            turn("assistant", "segunda"),
            turn("user", "terceira"),
        ];
        let mut budget = Budget { remaining: 10_000 };

        let kept = fit_history(history, &mut budget);

        let contents: Vec<&str> = kept.iter().map(|m| m.content.as_str()).collect();
        assert_eq!(contents, vec!["primeira", "segunda", "terceira"]);
    }

    #[test]
    fn the_retrieved_passages_sit_immediately_above_the_question() {
        let question = question_with_context(
            "continue a frase",
            &[source_block("codigo.pdf", "Art. 968. A inscrição do")],
        );

        assert!(
            question.ends_with("continue a frase"),
            "a pergunta tem que ser a última coisa que o modelo lê"
        );
        assert!(question.contains("Art. 968. A inscrição do"));
        assert!(question.starts_with(CONTEXT_PREAMBLE));
    }

    #[test]
    fn a_question_without_retrieval_carries_no_preamble() {
        // Told to cite sources with none given, a small model invents them —
        // observed as "[fonte: GPT-3 informações geral]".
        assert_eq!(question_with_context("o que é rust?", &[]), "o que é rust?");
    }

    #[test]
    fn every_context_block_names_its_source() {
        let block = source_block("contrato.pdf", "o prazo é de 30 dias");
        assert!(block.starts_with("[fonte: contrato.pdf]"));
        assert!(block.contains("o prazo é de 30 dias"));
    }

    #[test]
    fn context_budget_leaves_room_for_the_answer() {
        let chars = budget_chars(Some(1024));
        assert_eq!(chars, (1024 - 512) * CHARS_PER_TOKEN);
        assert!(budget_chars(Some(100)) == 0, "a tiny window reserves nothing");
    }
}
