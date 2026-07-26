use super::store::{EmbeddedChunk, VectorStore};
use super::{chunking, embedding, parsing, CHUNK_MAX_TOKENS, CHUNK_OVERLAP_TOKENS};
use crate::db::DbState;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

/// The document's position in the pipeline. Only `Ready` is searchable, and
/// everything before it is resumable — the app re-queues them at startup.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Queued,
    Parsing,
    Chunking,
    Embedding,
    Ready,
    Error,
}

impl DocumentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentStatus::Queued => "queued",
            DocumentStatus::Parsing => "parsing",
            DocumentStatus::Chunking => "chunking",
            DocumentStatus::Embedding => "embedding",
            DocumentStatus::Ready => "ready",
            DocumentStatus::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentStatusEvent {
    pub id: String,
    pub status: DocumentStatus,
    pub error_message: Option<String>,
}

fn set_status(
    app: &AppHandle,
    doc_id: &str,
    status: DocumentStatus,
    error_message: Option<String>,
) -> Result<(), String> {
    {
        let db = app.state::<DbState>();
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = crate::db::require_conn(&guard)?;
        sql.execute(
            "UPDATE documents SET status = ?1, error_message = ?2, updated_at = ?3 WHERE id = ?4",
            params![
                status.as_str(),
                error_message,
                chrono::Utc::now().to_rfc3339(),
                doc_id
            ],
        )
        .map_err(|e| e.to_string())?;
    }

    let _ = app.emit(
        "document-status",
        DocumentStatusEvent {
            id: doc_id.to_string(),
            status,
            error_message,
        },
    );
    Ok(())
}

/// A document deleted mid-processing must not resurrect: every stage checks
/// whether the row still exists before doing expensive work.
fn still_exists(app: &AppHandle, doc_id: &str) -> bool {
    let db = app.state::<DbState>();
    let Ok(guard) = db.0.lock() else { return false };
    let Some(sql) = guard.as_ref() else {
        return false;
    };
    sql.query_row(
        "SELECT 1 FROM documents WHERE id = ?1",
        params![doc_id],
        |_| Ok(()),
    )
    .optional()
    .map(|found| found.is_some())
    .unwrap_or(false)
}

pub fn vectors_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let cfg = crate::config::load_config(app)?
        .ok_or_else(|| "Nenhuma pasta de armazenamento configurada ainda".to_string())?;
    Ok(cfg.base_path_buf().join("vectors"))
}

/// Runs parse → chunk → embed → store for one document, moving its status
/// forward at each step. Errors become an `error` status with the message
/// instead of propagating: one bad file must not take down the queue.
pub async fn process_document(
    app: AppHandle,
    doc_id: String,
    file_path: PathBuf,
    namespace: String,
) {
    if let Err(message) = run_pipeline(&app, &doc_id, &file_path, &namespace).await {
        // The document may have been deleted on purpose mid-run; in that case
        // there is nothing left to mark as failed.
        if still_exists(&app, &doc_id) {
            let _ = set_status(&app, &doc_id, DocumentStatus::Error, Some(message));
        }
    }
}

async fn run_pipeline(
    app: &AppHandle,
    doc_id: &str,
    file_path: &PathBuf,
    namespace: &str,
) -> Result<(), String> {
    if !still_exists(app, doc_id) {
        return Ok(());
    }
    set_status(app, doc_id, DocumentStatus::Parsing, None)?;
    // Downloads the pdfium library on the first PDF; a no-op otherwise.
    super::pdfium::ensure_for(app, file_path).await?;
    let text = parsing::extract_text(file_path).map_err(|e| e.to_string())?;

    if !still_exists(app, doc_id) {
        return Ok(());
    }
    set_status(app, doc_id, DocumentStatus::Chunking, None)?;
    let chunks = chunking::chunk_text(&text, CHUNK_MAX_TOKENS, CHUNK_OVERLAP_TOKENS);
    if chunks.is_empty() {
        return Err("nenhum texto utilizável após o chunking".to_string());
    }

    if !still_exists(app, doc_id) {
        return Ok(());
    }
    set_status(app, doc_id, DocumentStatus::Embedding, None)?;
    // Downloads the ONNX Runtime on first use; a no-op afterwards.
    super::onnxruntime::ensure_dylib(app).await?;
    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    // Embedding is CPU-bound and synchronous; keeping it on the async worker
    // would stall every other task sharing that thread.
    let vectors = tauri::async_runtime::spawn_blocking(move || embedding::embed_passages(&texts))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    let embedded: Vec<EmbeddedChunk> = chunks
        .iter()
        .zip(vectors)
        .map(|(chunk, vector)| EmbeddedChunk {
            id: Uuid::new_v4().to_string(),
            text: chunk.text.clone(),
            vector,
            chunk_index: chunk.index as i32,
        })
        .collect();

    if !still_exists(app, doc_id) {
        return Ok(());
    }
    let store = VectorStore::open(&vectors_dir(app)?)
        .await
        .map_err(|e| e.to_string())?;
    store
        .upsert(namespace, doc_id, embedded)
        .await
        .map_err(|e| e.to_string())?;

    // A delete that lands during the write leaves orphan chunks behind, so
    // the last check cleans them up instead of leaving them searchable.
    if !still_exists(app, doc_id) {
        let _ = store.delete_by_doc(namespace, doc_id).await;
        return Ok(());
    }

    set_status(app, doc_id, DocumentStatus::Ready, None)
}
