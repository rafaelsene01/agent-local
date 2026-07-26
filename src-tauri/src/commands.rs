use crate::db::{require_conn, DbState};
use crate::models::{Chat, Message};
use chrono::Utc;
use rusqlite::params;
use tauri::{AppHandle, State};
use uuid::Uuid;

#[tauri::command]
pub fn create_chat(db: State<DbState>, title: Option<String>) -> Result<Chat, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = require_conn(&guard)?;
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let title = title
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "New chat".to_string());

    conn.execute(
        "INSERT INTO chats (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![id, title, now],
    )
    .map_err(|e| e.to_string())?;

    Ok(Chat {
        id,
        title,
        created_at: now.clone(),
        updated_at: now,
        // Matches the column default; a new chat starts using the global base.
        use_global_rag: true,
    })
}

fn row_to_chat(row: &rusqlite::Row) -> rusqlite::Result<Chat> {
    Ok(Chat {
        id: row.get(0)?,
        title: row.get(1)?,
        created_at: row.get(2)?,
        updated_at: row.get(3)?,
        use_global_rag: row.get::<_, i64>(4)? != 0,
    })
}

const SELECT_CHAT: &str = "SELECT id, title, created_at, updated_at, use_global_rag FROM chats";

#[tauri::command]
pub fn list_chats(db: State<DbState>) -> Result<Vec<Chat>, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = require_conn(&guard)?;
    let mut stmt = conn
        .prepare(&format!("{SELECT_CHAT} ORDER BY updated_at DESC"))
        .map_err(|e| e.to_string())?;

    let chats = stmt
        .query_map([], row_to_chat)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<Chat>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(chats)
}

#[tauri::command]
pub fn rename_chat(db: State<DbState>, id: String, title: String) -> Result<Chat, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = require_conn(&guard)?;
    let title = title.trim();
    if title.is_empty() {
        return Err("O título não pode ficar vazio".to_string());
    }
    let now = Utc::now().to_rfc3339();

    let updated = conn
        .execute(
            "UPDATE chats SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![title, now, id],
        )
        .map_err(|e| e.to_string())?;

    if updated == 0 {
        return Err("Chat não encontrado".to_string());
    }

    conn.query_row(
        &format!("{SELECT_CHAT} WHERE id = ?1"),
        params![id],
        row_to_chat,
    )
    .map_err(|e| e.to_string())
}

/// Deleting a chat takes its attachments with it (AD-004/CHAT-12): the rows,
/// the files under `chats/<id>/tmp/` and the chat's vector namespace. The
/// filesystem and vector cleanup run outside the transaction because neither
/// can participate in it — a leftover file is recoverable, a lost chat isn't.
#[tauri::command]
pub async fn delete_chat(
    app: AppHandle,
    db: State<'_, DbState>,
    id: String,
) -> Result<(), String> {
    {
        let mut guard = db.0.lock().map_err(|e| e.to_string())?;
        let conn = guard
            .as_mut()
            .ok_or_else(|| "Nenhuma pasta de armazenamento configurada ainda".to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM messages WHERE chat_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        tx.execute("DELETE FROM chat_attachments WHERE chat_id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        let deleted = tx
            .execute("DELETE FROM chats WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;

        if deleted == 0 {
            return Err("Chat não encontrado".to_string());
        }
    }

    if let Ok(Some(cfg)) = crate::config::load_config(&app) {
        let _ = std::fs::remove_dir_all(cfg.base_path_buf().join("chats").join(&id));
    }

    if let Ok(dir) = crate::rag::pipeline::vectors_dir(&app) {
        if let Ok(store) = crate::rag::store::VectorStore::open(&dir).await {
            let _ = store
                .delete_namespace(&crate::rag::store::chat_namespace(&id))
                .await;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn list_messages(db: State<DbState>, chat_id: String) -> Result<Vec<Message>, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let conn = require_conn(&guard)?;
    let mut stmt = conn
        .prepare(
            "SELECT id, chat_id, role, content, created_at FROM messages WHERE chat_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;

    let messages = stmt
        .query_map(params![chat_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                chat_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<Message>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(messages)
}
