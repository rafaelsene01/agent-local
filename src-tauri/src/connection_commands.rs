use crate::connections::{self, Connection, ConnectionStatus};
use crate::db::{require_conn, DbState};
use crate::embedded_commands;
use tauri::{AppHandle, State};

/// Seeds the known candidates (inactive) the first time they're missing, then
/// live-checks every connection's status — CONN-01 AC1 tests all of them
/// regardless of which is active, since the user needs to see status before
/// deciding which one to pick.
#[tauri::command]
pub async fn list_connections(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<Vec<Connection>, String> {
    let manager = embedded_commands::manager(&app);

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
pub fn set_active_connection(app: AppHandle, db: State<DbState>, id: String) -> Result<(), String> {
    let activated_embedded = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        connections::set_active_connection(sql, &id)?;
        connections::active_connection(sql)?.is_some_and(|c| c.provider == "embedded")
    };

    // Switching away from the embedded runtime stops its process: keeping a
    // local server alive for a connection nothing talks to is the resource
    // leak EMBED-09 exists to prevent.
    if !activated_embedded {
        stop_sidecar(&app)?;
    }
    Ok(())
}

#[tauri::command]
pub fn clear_active_connection(app: AppHandle, db: State<DbState>) -> Result<(), String> {
    {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        connections::clear_active_connection(sql)?;
    }
    stop_sidecar(&app)
}

fn stop_sidecar(app: &AppHandle) -> Result<(), String> {
    embedded_commands::stop_embedded_runtime(app.clone())
}

#[tauri::command]
pub async fn refresh_connection_status(
    app: AppHandle,
    db: State<'_, DbState>,
    id: String,
) -> Result<ConnectionStatus, String> {
    let manager = embedded_commands::manager(&app);
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
