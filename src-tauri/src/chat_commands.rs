use crate::chat::attachments;
use crate::chat::cancellation::CancellationRegistry;
use crate::chat::context_assembler;
use crate::connections;
use crate::db::{require_conn, DbState};
use crate::embedded_commands;
use crate::models::Message;
use crate::providers::GpuOffload;
use chrono::Utc;
use futures_util::StreamExt;
use rusqlite::{params, OptionalExtension};
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

struct ActiveTarget {
    connection: connections::Connection,
    model_name: String,
    context_length: Option<u32>,
    gpu_offload: Option<GpuOffload>,
}

/// The single active pair is the only source of truth for who answers
/// (AD-021). Missing either half is a clear error before any network call
/// (CHAT-02).
fn active_target(app: &AppHandle) -> Result<ActiveTarget, String> {
    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;

    let connection = connections::active_connection(sql)?
        .ok_or_else(|| "Nenhuma conexão ativa — escolha uma em Conexões".to_string())?;

    let row: Option<(String, Option<u32>, Option<String>)> = sql
        .query_row(
            "SELECT model_name, context_length, gpu_offload FROM model_configs WHERE is_active = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let (model_name, context_length, gpu_offload) =
        row.ok_or_else(|| "Nenhum modelo ativo — escolha um em Conexões".to_string())?;

    Ok(ActiveTarget {
        connection,
        model_name,
        context_length,
        gpu_offload: gpu_offload.as_deref().map(GpuOffload::parse).transpose()?,
    })
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
    let target = active_target(&app)?;
    let user_message = insert_message(&app, &chat_id, "user", &content)?;

    // Attachment failures are recorded per file and never block the message.
    attachments::ingest(&app, &chat_id, &attachment_paths).await?;

    let messages = context_assembler::assemble(
        &app,
        &chat_id,
        &content,
        use_global_rag(&app, &chat_id)?,
        target.context_length,
    )
    .await?;

    let token = app
        .state::<CancellationRegistry>()
        .register(&chat_id);
    let assistant_message_id = Uuid::new_v4().to_string();
    let manager = embedded_commands::manager(&app);
    let client = manager.provider_for(&target.connection);

    let mut stream = match client
        .stream_chat(
            &target.model_name,
            messages,
            target.context_length,
            target.gpu_offload,
        )
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
