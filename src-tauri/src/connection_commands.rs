use crate::connections::{self, Connection, ConnectionManager, ConnectionStatus};
use crate::db::DbState;
use rusqlite::Connection as SqlConnection;
use tauri::State;

fn require_conn<'a>(
    guard: &'a std::sync::MutexGuard<'a, Option<SqlConnection>>,
) -> Result<&'a SqlConnection, String> {
    guard
        .as_ref()
        .ok_or_else(|| "Nenhuma pasta de armazenamento configurada ainda".to_string())
}

/// Seeds the known Ollama/LM Studio candidates (disabled by default) the
/// first time they're missing, then live-checks every connection's status —
/// CONN-01 AC1 tests both regardless of whether they're enabled yet, since
/// the user needs to see status before deciding to enable one.
#[tauri::command]
pub async fn list_connections(db: State<'_, DbState>) -> Result<Vec<Connection>, String> {
    let manager = ConnectionManager::new();

    let base_list = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        let existing = connections::list_connections(sql)?;
        for candidate in manager.detect_known_connections() {
            let already_present = existing.iter().any(|c| c.provider == candidate.provider);
            if !already_present {
                connections::create_connection(sql, candidate.provider, candidate.base_url, false)?;
            }
        }
        connections::list_connections(sql)?
    };

    let mut refreshed = Vec::with_capacity(base_list.len());
    for conn in base_list {
        let status = manager.refresh_status(&conn).await;
        refreshed.push(Connection { status, ..conn });
    }
    Ok(refreshed)
}

#[tauri::command]
pub fn add_connection(
    db: State<DbState>,
    provider: String,
    base_url: String,
) -> Result<Connection, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;
    connections::create_connection(sql, provider, base_url, true)
}

#[tauri::command]
pub fn toggle_connection(db: State<DbState>, id: String, enabled: bool) -> Result<(), String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;
    connections::toggle_connection(sql, &id, enabled)
}

#[tauri::command]
pub async fn refresh_connection_status(
    db: State<'_, DbState>,
    id: String,
) -> Result<ConnectionStatus, String> {
    let manager = ConnectionManager::new();
    let conn = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        connections::list_connections(sql)?
            .into_iter()
            .find(|c| c.id == id)
            .ok_or_else(|| "Conexão não encontrada".to_string())?
    };
    Ok(manager.refresh_status(&conn).await)
}
