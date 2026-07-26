use crate::db::{require_conn, DbState};
use crate::rag::parsing;
use crate::rag::pipeline::{self, DocumentStatus};
use crate::rag::store::{VectorStore, GLOBAL_NAMESPACE};
use chrono::Utc;
use rusqlite::params;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

/// Documents are copied into the base folder, so a 200MB file would be
/// duplicated on disk and take minutes to embed. The limit is generous for
/// text formats and still refuses obvious mistakes early (DOC-03).
const MAX_FILE_BYTES: u64 = 100 * 1024 * 1024;

#[derive(Debug, Serialize, Clone)]
pub struct DocumentRecord {
    pub id: String,
    pub filename: String,
    pub file_path: String,
    pub size_bytes: u64,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

fn documents_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let cfg = crate::config::load_config(app)?
        .ok_or_else(|| "Nenhuma pasta de armazenamento configurada ainda".to_string())?;
    Ok(cfg.base_path_buf().join("documents"))
}

/// Keeps the original name when possible; a second import of the same name
/// gets a suffix rather than overwriting the earlier document's bytes.
fn unique_destination(dir: &Path, filename: &str) -> PathBuf {
    let candidate = dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let stem = Path::new(filename)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "documento".to_string());
    let ext = Path::new(filename)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    for n in 2..1000 {
        let candidate = dir.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem}-{}{ext}", Uuid::new_v4()))
}

fn row_to_record(row: &rusqlite::Row) -> rusqlite::Result<DocumentRecord> {
    Ok(DocumentRecord {
        id: row.get(0)?,
        filename: row.get(1)?,
        file_path: row.get(2)?,
        size_bytes: row.get::<_, i64>(3)? as u64,
        status: row.get(4)?,
        error_message: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

const SELECT_DOCUMENT: &str =
    "SELECT id, filename, file_path, size_bytes, status, error_message, created_at, updated_at FROM documents";

/// A file the import refused, with the reason to show next to its name.
#[derive(Debug, Serialize, Clone)]
pub struct RejectedImport {
    pub path: String,
    pub reason: String,
}

/// One bad file in a selection must not throw away the good ones (DOC-03):
/// each is judged on its own and the rejected ones come back named.
#[derive(Debug, Serialize, Clone)]
pub struct ImportResult {
    pub imported: Vec<DocumentRecord>,
    pub rejected: Vec<RejectedImport>,
}

#[tauri::command]
pub fn import_documents(
    app: AppHandle,
    db: State<DbState>,
    paths: Vec<String>,
) -> Result<ImportResult, String> {
    let dir = documents_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let mut created = Vec::new();
    let mut rejected = Vec::new();
    let mut reject = |path: &PathBuf, reason: String| {
        rejected.push(RejectedImport {
            path: path.to_string_lossy().to_string(),
            reason,
        });
    };

    for raw in paths {
        let source = PathBuf::from(&raw);
        if !parsing::is_supported(&source) {
            reject(
                &source,
                "formato não suportado. Aceitos: PDF, DOCX, TXT, MD".to_string(),
            );
            continue;
        }
        let metadata = match std::fs::metadata(&source) {
            Ok(metadata) => metadata,
            Err(e) => {
                reject(&source, e.to_string());
                continue;
            }
        };
        if metadata.len() > MAX_FILE_BYTES {
            reject(
                &source,
                format!(
                    "tem {:.1} MB e excede o limite de {} MB",
                    metadata.len() as f64 / 1e6,
                    MAX_FILE_BYTES / 1024 / 1024
                ),
            );
            continue;
        }

        let Some(filename) = source
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
        else {
            reject(&source, "caminho de arquivo inválido".to_string());
            continue;
        };
        let destination = unique_destination(&dir, &filename);
        if let Err(e) = std::fs::copy(&source, &destination) {
            reject(&source, e.to_string());
            continue;
        }

        let record = DocumentRecord {
            id: Uuid::new_v4().to_string(),
            filename,
            file_path: destination.to_string_lossy().to_string(),
            size_bytes: metadata.len(),
            status: DocumentStatus::Queued.as_str().to_string(),
            error_message: None,
            created_at: Utc::now().to_rfc3339(),
            updated_at: Utc::now().to_rfc3339(),
        };

        {
            let guard = db.0.lock().map_err(|e| e.to_string())?;
            let sql = require_conn(&guard)?;
            sql.execute(
                "INSERT INTO documents (id, filename, file_path, size_bytes, status, error_message, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7)",
                params![
                    record.id,
                    record.filename,
                    record.file_path,
                    record.size_bytes as i64,
                    record.status,
                    record.created_at,
                    record.updated_at
                ],
            )
            .map_err(|e| e.to_string())?;
        }

        spawn_processing(&app, &record.id, &record.file_path);
        created.push(record);
    }

    Ok(ImportResult {
        imported: created,
        rejected,
    })
}

/// Each document gets its own task, so importing several keeps the UI
/// responsive and they progress independently (DOC-07).
fn spawn_processing(app: &AppHandle, doc_id: &str, file_path: &str) {
    let app = app.clone();
    let doc_id = doc_id.to_string();
    let path = PathBuf::from(file_path);
    tauri::async_runtime::spawn(async move {
        pipeline::process_document(app, doc_id, path, GLOBAL_NAMESPACE.to_string()).await;
    });
}

#[tauri::command]
pub fn list_documents(db: State<DbState>) -> Result<Vec<DocumentRecord>, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;
    let mut stmt = sql
        .prepare(&format!("{SELECT_DOCUMENT} ORDER BY created_at DESC"))
        .map_err(|e| e.to_string())?;
    let documents = stmt
        .query_map([], row_to_record)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(documents)
}

#[tauri::command]
pub async fn delete_document(
    app: AppHandle,
    db: State<'_, DbState>,
    id: String,
) -> Result<(), String> {
    let file_path: Option<String> = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        let path: Option<String> = sql
            .query_row(
                "SELECT file_path FROM documents WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .ok();
        // Removing the row first is what tells a running pipeline to abort.
        let deleted = sql
            .execute("DELETE FROM documents WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        if deleted == 0 {
            return Err("Documento não encontrado".to_string());
        }
        path
    };

    if let Some(path) = file_path {
        let _ = std::fs::remove_file(path);
    }

    let store = VectorStore::open(&pipeline::vectors_dir(&app)?)
        .await
        .map_err(|e| e.to_string())?;
    store
        .delete_by_doc(GLOBAL_NAMESPACE, &id)
        .await
        .map_err(|e| e.to_string())
}

/// A crash or a quit during processing leaves documents parked in a
/// non-terminal status. They are re-run from the start at boot — cheap
/// enough that checkpointing mid-pipeline isn't worth the complexity.
pub fn requeue_unfinished_documents(app: &AppHandle) {
    let pending: Vec<(String, String)> = {
        let db = app.state::<DbState>();
        let Ok(guard) = db.0.lock() else { return };
        let Some(sql) = guard.as_ref() else { return };
        let Ok(mut stmt) = sql.prepare(
            "SELECT id, file_path FROM documents WHERE status IN ('queued','parsing','chunking','embedding')",
        ) else {
            return;
        };
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)));
        match rows {
            Ok(rows) => rows.filter_map(Result::ok).collect(),
            Err(_) => return,
        }
    };

    for (id, path) in pending {
        spawn_processing(app, &id, &path);
    }
}
