use crate::chat::attachments;
use crate::chat::cancellation::CancellationRegistry;
use crate::chat::context_assembler;
use crate::db::{require_conn, DbState};
use crate::runtime_commands;
use crate::models::Message;
use chrono::Utc;
use futures_util::StreamExt;
use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

#[derive(Debug, Serialize, Clone)]
pub struct ChatStreamChunk {
    pub chat_id: String,
    pub message_id: String,
    pub delta: String,
    pub done: bool,
    pub error: Option<String>,
}

/// Emitted when the answer was produced without the knowledge base because
/// retrieval failed. Separate from `ChatStreamChunk.error`, which means the
/// message itself failed.
#[derive(Debug, Serialize, Clone)]
pub struct ChatRetrievalWarning {
    pub chat_id: String,
    pub reason: String,
}

fn insert_message(
    app: &AppHandle,
    chat_id: &str,
    role: &str,
    content: &str,
) -> Result<Message, String> {
    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;
    let message = Message {
        id: Uuid::new_v4().to_string(),
        chat_id: chat_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        created_at: Utc::now().to_rfc3339(),
    };
    sql.execute(
        "INSERT INTO messages (id, chat_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            message.id,
            message.chat_id,
            message.role,
            message.content,
            message.created_at
        ],
    )
    .map_err(|e| e.to_string())?;
    // Keeps the chat at the top of the list, which sorts by updated_at.
    sql.execute(
        "UPDATE chats SET updated_at = ?1 WHERE id = ?2",
        params![message.created_at, chat_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(message)
}

#[tauri::command]
pub fn create_message(
    app: AppHandle,
    chat_id: String,
    role: String,
    content: String,
) -> Result<Message, String> {
    insert_message(&app, &chat_id, &role, &content)
}

#[tauri::command]
pub fn set_chat_use_global_rag(
    db: State<DbState>,
    chat_id: String,
    enabled: bool,
) -> Result<(), String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;
    sql.execute(
        "UPDATE chats SET use_global_rag = ?1 WHERE id = ?2",
        params![enabled as i64, chat_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// What the chat shows about each file the user attached. `extracted_text` is
/// deliberately left out: it can be thousands of characters and the UI only
/// needs to say whether the file made it into the context (CHAT-10).
#[derive(Debug, Serialize, Clone)]
pub struct ChatAttachment {
    pub id: String,
    pub filename: String,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub fn list_chat_attachments(
    db: State<DbState>,
    chat_id: String,
) -> Result<Vec<ChatAttachment>, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;
    let mut stmt = sql
        .prepare(
            "SELECT id, filename, status, error_message, created_at FROM chat_attachments
             WHERE chat_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![chat_id], |row| {
            Ok(ChatAttachment {
                id: row.get(0)?,
                filename: row.get(1)?,
                status: row.get(2)?,
                error_message: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// There is one runtime, so "who answers" is no longer a pair to resolve — it
/// is whatever model the runtime is configured to run (SELF-04). The error
/// names the screen that fixes it, which is the only thing the user can do
/// about it (CHAT-02).
fn active_model(app: &AppHandle) -> Result<crate::runtime::store::ActiveModel, String> {
    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;

    crate::runtime::store::active_model(sql)?
        .ok_or_else(|| "Nenhum modelo ativo — escolha um em Runtime".to_string())
}

/// The window to budget the prompt against. A missing `context_length` used to
/// mean "assume 4096", which cost the runtime four fifths of its real window —
/// the sidecar reports `n_ctx_slot = 21760` for Phi-3.5, and the document
/// context was being truncated to fit a limit that did not exist (AD-033).
///
/// Best-effort on purpose: a runtime that cannot answer falls back to the old
/// assumption instead of failing the message.
async fn budget_context(
    client: &crate::providers::llama_server::LlamaServerClient,
    model: &crate::runtime::store::ActiveModel,
) -> Option<u32> {
    if model.context_length.is_some() {
        return model.context_length;
    }
    client
        .model_limits(&model.name)
        .await
        .ok()
        .and_then(|limits| limits.current_context)
}

fn use_global_rag(app: &AppHandle, chat_id: &str) -> Result<bool, String> {
    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;
    let enabled: i64 = sql
        .query_row(
            "SELECT use_global_rag FROM chats WHERE id = ?1",
            params![chat_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(enabled != 0)
}

/// Returns the user's message id right away; the answer arrives as
/// `chat-stream-chunk` events, because Tauri commands are request/response
/// and token-by-token output needs push (AD-018).
#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    chat_id: String,
    content: String,
    attachment_paths: Vec<String>,
) -> Result<String, String> {
    let model = active_model(&app)?;
    let user_message = insert_message(&app, &chat_id, "user", &content)?;

    // Attachment failures are recorded per file and never block the message.
    attachments::ingest(&app, &chat_id, &attachment_paths).await?;

    let client = runtime_commands::client(&app);

    let assembled = context_assembler::assemble(
        &app,
        &chat_id,
        &content,
        &user_message.id,
        use_global_rag(&app, &chat_id)?,
        budget_context(&client, &model).await,
    )
    .await?;

    // The answer still goes out — it is just answered without the documents,
    // and the user gets told so instead of having to guess whether the model
    // ignored the knowledge base or the knowledge base failed.
    if let Some(reason) = assembled.retrieval_error {
        let _ = app.emit(
            "chat-retrieval-warning",
            ChatRetrievalWarning {
                chat_id: chat_id.clone(),
                reason,
            },
        );
    }

    let token = app
        .state::<CancellationRegistry>()
        .register(&chat_id);
    let assistant_message_id = Uuid::new_v4().to_string();

    let mut stream = match client
        .stream_chat(&model.name, assembled.messages, model.context_length)
        .await
    {
        Ok(stream) => stream,
        Err(e) => {
            app.state::<CancellationRegistry>().finish(&chat_id);
            return Err(e.to_string());
        }
    };

    let mut accumulated = String::new();
    let mut error: Option<String> = None;

    while let Some(item) = stream.next().await {
        if token.is_cancelled() {
            break;
        }
        match item {
            Ok(chunk) => {
                if !chunk.delta.is_empty() {
                    accumulated.push_str(&chunk.delta);
                    let _ = app.emit(
                        "chat-stream-chunk",
                        ChatStreamChunk {
                            chat_id: chat_id.clone(),
                            message_id: assistant_message_id.clone(),
                            delta: chunk.delta,
                            done: false,
                            error: None,
                        },
                    );
                }
                if chunk.done {
                    break;
                }
            }
            Err(e) => {
                error = Some(e.to_string());
                break;
            }
        }
    }

    app.state::<CancellationRegistry>().finish(&chat_id);

    // Whatever arrived is kept: a cancelled or failed generation still leaves
    // the user with the part that was already on screen (CHAT-04, CHAT-05).
    if !accumulated.is_empty() {
        let _ = insert_message(&app, &chat_id, "assistant", &accumulated);
    }

    let _ = app.emit(
        "chat-stream-chunk",
        ChatStreamChunk {
            chat_id: chat_id.clone(),
            message_id: assistant_message_id,
            delta: String::new(),
            done: true,
            error: error.clone(),
        },
    );

    match error {
        Some(e) => Err(e),
        None => Ok(user_message.id),
    }
}

#[tauri::command]
pub fn cancel_generation(app: AppHandle, chat_id: String) -> Result<(), String> {
    app.state::<CancellationRegistry>().cancel(&chat_id);
    Ok(())
}
