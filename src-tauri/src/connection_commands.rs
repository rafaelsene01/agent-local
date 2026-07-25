use crate::connections::{self, Connection, ConnectionManager, ConnectionStatus};
use crate::db::{require_conn, DbState};
use tauri::State;

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
                connections::create_connection(sql, candidate.provider, candidate.base_url)?;
            }
        }
        connections::list_connections(sql)?
    };

    // Checked concurrently, but collected in the original order: the list
    // must not reshuffle depending on which runtime answered first (C-02).
    let refreshed = futures_util::future::join_all(base_list.into_iter().map(|conn| {
        let manager = &manager;
        async move {
            let status = manager.refresh_status(&conn).await;
            Connection { status, ..conn }
        }
    }))
    .await;
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
    connections::create_connection(sql, provider, base_url)
}

/// Activating an unreachable connection is allowed on purpose (ACTIVE-04):
/// the user's choice is respected and `status` reports the reality instead of
/// the app silently picking a different runtime.
#[tauri::command]
pub fn set_active_connection(db: State<DbState>, id: String) -> Result<(), String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;
    connections::set_active_connection(sql, &id)
}

#[tauri::command]
pub fn clear_active_connection(db: State<DbState>) -> Result<(), String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;
    connections::clear_active_connection(sql)
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
