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

/// Only the global knowledge base. Chat attachments borrow this table for the
/// duration of their indexing (see `chat::attachments`), and those rows are not
/// documents the user imported — showing them would put a phantom file in the
/// Documentos tab, pointing inside a chat's temp folder.
const SELECT_DOCUMENT: &str =
    "SELECT id, filename, file_path, size_bytes, status, error_message, created_at, updated_at
     FROM documents WHERE namespace = 'global'";

/// Documents whose processing can be resumed at boot. The `namespace` filter is
/// the whole point: without it, a chat attachment's borrowed row comes back as
/// a global document.
const SELECT_RESUMABLE: &str =
    "SELECT id, file_path FROM documents
     WHERE status IN ('queued','parsing','chunking','embedding') AND namespace = 'global'";

const FAIL_INTERRUPTED_ATTACHMENTS: &str =
    "UPDATE chat_attachments SET status = 'error',
            error_message = 'a indexação foi interrompida quando o app fechou; envie o arquivo de novo'
     WHERE id IN (SELECT id FROM documents WHERE namespace <> 'global')
       AND status NOT IN ('ready', 'injected_whole', 'error')";

const DELETE_BORROWED_ROWS: &str = "DELETE FROM documents WHERE namespace <> 'global'";

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
    // Only the global base is resumable. A row from another namespace is the
    // leftover of a chat attachment whose indexing was interrupted, and
    // re-running it here would index a private file into the global base — the
    // namespace is not even carried over, so it would land under 'global'
    // and become visible to every chat (CHAT-11).
    let pending: Vec<(String, String)> = {
        let db = app.state::<DbState>();
        let Ok(guard) = db.0.lock() else { return };
        let Some(sql) = guard.as_ref() else { return };
        let Ok(mut stmt) = sql.prepare(SELECT_RESUMABLE) else {
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

    discard_interrupted_attachments(app);
}

/// Clears the temporary rows a chat attachment leaves in `documents` when the
/// app dies mid-indexing, and marks the attachment itself as failed.
///
/// Silence would be worse than an error here: the attachment is already
/// recorded as `queued` in `chat_attachments` and the chat would show it as
/// accepted forever, while nothing was ever indexed.
fn discard_interrupted_attachments(app: &AppHandle) {
    let db = app.state::<DbState>();
    let Ok(guard) = db.0.lock() else { return };
    let Some(sql) = guard.as_ref() else { return };

    let _ = sql.execute(FAIL_INTERRUPTED_ATTACHMENTS, []);
    let _ = sql.execute(DELETE_BORROWED_ROWS, []);
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn migrated() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::db::apply_migrations(&mut conn).unwrap();
        conn
    }

    fn insert_document(conn: &Connection, id: &str, status: &str, namespace: &str) {
        conn.execute(
            "INSERT INTO documents (id, filename, file_path, size_bytes, status, error_message, created_at, updated_at, namespace)
             VALUES (?1, 'f.pdf', '/tmp/f.pdf', 1, ?2, NULL, 'now', 'now', ?3)",
            params![id, status, namespace],
        )
        .unwrap();
    }

    fn resumable_ids(conn: &Connection) -> Vec<String> {
        let mut stmt = conn.prepare(SELECT_RESUMABLE).unwrap();
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        ids
    }

    #[test]
    fn a_chat_attachment_is_never_resumed_as_a_global_document() {
        // The regression this exists for: the app killed while indexing a large
        // attachment left a 'queued' row behind, and the boot-time requeue
        // re-indexed that private file into the global base (CHAT-11).
        let conn = migrated();
        insert_document(&conn, "doc-global", "queued", "global");
        insert_document(&conn, "att-chat", "embedding", "chat:abc");

        assert_eq!(resumable_ids(&conn), vec!["doc-global".to_string()]);
    }

    #[test]
    fn finished_documents_are_not_resumed() {
        let conn = migrated();
        insert_document(&conn, "ready", "ready", "global");
        insert_document(&conn, "failed", "error", "global");
        insert_document(&conn, "midway", "chunking", "global");

        assert_eq!(resumable_ids(&conn), vec!["midway".to_string()]);
    }

    #[test]
    fn interrupted_attachments_are_reported_and_their_borrowed_rows_removed() {
        let conn = migrated();
        conn.execute(
            "INSERT INTO chats (id, title, created_at, updated_at) VALUES ('c1', 't', 'now', 'now')",
            [],
        )
        .unwrap();
        insert_document(&conn, "att-1", "embedding", "chat:c1");
        conn.execute(
            "INSERT INTO chat_attachments (id, chat_id, filename, file_path, size_bytes, status, created_at)
             VALUES ('att-1', 'c1', 'grande.pdf', '/tmp/grande.pdf', 99, 'queued', 'now')",
            [],
        )
        .unwrap();

        conn.execute(FAIL_INTERRUPTED_ATTACHMENTS, []).unwrap();
        conn.execute(DELETE_BORROWED_ROWS, []).unwrap();

        let (status, error): (String, Option<String>) = conn
            .query_row(
                "SELECT status, error_message FROM chat_attachments WHERE id = 'att-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(status, "error", "a silent 'queued' forever would be worse");
        assert!(error.unwrap().contains("interrompida"));

        let leftover: i64 = conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))
            .unwrap();
        assert_eq!(leftover, 0);
    }

    #[test]
    fn an_attachment_that_finished_is_left_alone() {
        let conn = migrated();
        conn.execute(
            "INSERT INTO chats (id, title, created_at, updated_at) VALUES ('c1', 't', 'now', 'now')",
            [],
        )
        .unwrap();
        insert_document(&conn, "att-ok", "ready", "chat:c1");
        conn.execute(
            "INSERT INTO chat_attachments (id, chat_id, filename, file_path, size_bytes, status, created_at)
             VALUES ('att-ok', 'c1', 'ok.pdf', '/tmp/ok.pdf', 10, 'ready', 'now')",
            [],
        )
        .unwrap();

        conn.execute(FAIL_INTERRUPTED_ATTACHMENTS, []).unwrap();

        let status: String = conn
            .query_row(
                "SELECT status FROM chat_attachments WHERE id = 'att-ok'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(status, "ready");
    }

    #[test]
    fn the_documents_list_shows_only_the_global_base() {
        let conn = migrated();
        insert_document(&conn, "doc-global", "ready", "global");
        insert_document(&conn, "att-chat", "ready", "chat:abc");

        let mut stmt = conn.prepare(SELECT_DOCUMENT).unwrap();
        let ids = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(ids, vec!["doc-global".to_string()]);
    }
}
