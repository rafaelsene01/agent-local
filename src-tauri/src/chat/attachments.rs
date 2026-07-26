use crate::db::DbState;
use crate::rag::parsing;
use crate::rag::pipeline;
use crate::rag::store::chat_namespace;
use chrono::Utc;
use rusqlite::params;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};
use uuid::Uuid;

/// Below this, the file goes into the prompt whole (CHAT-09): chunking and
/// embedding a short note costs more than it gains, and retrieval could even
/// drop the one line that mattered. ~2000 tokens at ~4 chars per token.
const WHOLE_INJECTION_MAX_CHARS: usize = 8000;

fn attachments_dir(app: &AppHandle, chat_id: &str) -> Result<PathBuf, String> {
    let cfg = crate::config::load_config(app)?
        .ok_or_else(|| "Nenhuma pasta de armazenamento configurada ainda".to_string())?;
    Ok(cfg.base_path_buf().join("chats").join(chat_id).join("tmp"))
}

fn record_attachment(
    app: &AppHandle,
    id: &str,
    chat_id: &str,
    filename: &str,
    path: &str,
    size: u64,
    status: &str,
    extracted_text: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = crate::db::require_conn(&guard)?;
    sql.execute(
        "INSERT INTO chat_attachments (id, chat_id, message_id, filename, file_path, size_bytes, status, extracted_text, error_message, created_at)
         VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(id) DO UPDATE SET status = excluded.status,
                                       extracted_text = excluded.extracted_text,
                                       error_message = excluded.error_message",
        params![
            id,
            chat_id,
            filename,
            path,
            size as i64,
            status,
            extracted_text,
            error,
            Utc::now().to_rfc3339()
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Copies each attachment into the chat's own folder and makes its content
/// usable *before* the answer is generated — the question being asked right
/// now is the one that needs it (CHAT-08).
///
/// A file that fails to process is recorded as `error` and skipped; the text
/// message is still sent (CHAT-10).
pub async fn ingest(app: &AppHandle, chat_id: &str, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let dir = attachments_dir(app, chat_id)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    for raw in paths {
        let source = PathBuf::from(raw);
        let filename = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "anexo".to_string());
        let id = Uuid::new_v4().to_string();
        let destination = dir.join(&filename);

        let size = match std::fs::copy(&source, &destination) {
            Ok(size) => size,
            Err(e) => {
                let _ = record_attachment(
                    app,
                    &id,
                    chat_id,
                    &filename,
                    &destination.to_string_lossy(),
                    0,
                    "error",
                    None,
                    Some(&e.to_string()),
                );
                continue;
            }
        };
        let path_str = destination.to_string_lossy().to_string();

        if !parsing::is_supported(&destination) {
            let _ = record_attachment(
                app,
                &id,
                chat_id,
                &filename,
                &path_str,
                size,
                "error",
                None,
                Some("formato não suportado"),
            );
            continue;
        }

        if let Err(e) = crate::rag::pdfium::ensure_for(app, &destination).await {
            let _ = record_attachment(
                app, &id, chat_id, &filename, &path_str, size, "error", None, Some(&e),
            );
            continue;
        }

        match parsing::extract_text(&destination) {
            Ok(text) if text.len() <= WHOLE_INJECTION_MAX_CHARS => {
                record_attachment(
                    app,
                    &id,
                    chat_id,
                    &filename,
                    &path_str,
                    size,
                    "injected_whole",
                    Some(&text),
                    None,
                )?;
            }
            Ok(_) => {
                record_attachment(
                    app, &id, chat_id, &filename, &path_str, size, "queued", None, None,
                )?;
                // Awaited on purpose: the current question must be able to use
                // the attachment, so the send waits for indexing to finish.
                index_large_attachment(app, chat_id, &id, &destination).await?;
            }
            Err(e) => {
                let _ = record_attachment(
                    app,
                    &id,
                    chat_id,
                    &filename,
                    &path_str,
                    size,
                    "error",
                    None,
                    Some(&e.to_string()),
                );
            }
        }
    }
    Ok(())
}

/// Reuses the documents pipeline with the chat's namespace instead of
/// duplicating parse → chunk → embed → store (AD-017). The pipeline tracks
/// its state in `documents`, so a temporary row stands in for the attachment
/// and is removed afterwards, leaving `chat_attachments` as the record.
async fn index_large_attachment(
    app: &AppHandle,
    chat_id: &str,
    attachment_id: &str,
    path: &PathBuf,
) -> Result<(), String> {
    {
        let db = app.state::<DbState>();
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = crate::db::require_conn(&guard)?;
        sql.execute(
            "INSERT INTO documents (id, filename, file_path, size_bytes, status, error_message, created_at, updated_at)
             VALUES (?1, ?2, ?3, 0, 'queued', NULL, ?4, ?4)",
            params![
                attachment_id,
                path.file_name().map(|n| n.to_string_lossy().to_string()),
                path.to_string_lossy(),
                Utc::now().to_rfc3339()
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    pipeline::process_document(
        app.clone(),
        attachment_id.to_string(),
        path.clone(),
        chat_namespace(chat_id),
    )
    .await;

    let (status, error) = {
        let db = app.state::<DbState>();
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = crate::db::require_conn(&guard)?;
        let result: (String, Option<String>) = sql
            .query_row(
                "SELECT status, error_message FROM documents WHERE id = ?1",
                params![attachment_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| e.to_string())?;
        sql.execute("DELETE FROM documents WHERE id = ?1", params![attachment_id])
            .map_err(|e| e.to_string())?;
        result
    };

    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = crate::db::require_conn(&guard)?;
    sql.execute(
        "UPDATE chat_attachments SET status = ?1, error_message = ?2 WHERE id = ?3",
        params![status, error, attachment_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
