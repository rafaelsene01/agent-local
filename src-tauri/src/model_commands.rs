use crate::connections::{self, Connection};
use crate::embedded_commands;
use crate::db::{require_conn, DbState};
use crate::models::catalog::{curated_models, CuratedModelInfo};
use crate::providers::{ConfigApplied, GpuOffload, InstalledModel, PullProgress};
use crate::system_info;
use rusqlite::{params, Connection as SqlConnection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

fn get_or_create_model_config(
    sql: &SqlConnection,
    connection_id: &str,
    model_name: &str,
) -> Result<String, String> {
    let existing: Option<String> = sql
        .query_row(
            "SELECT id FROM model_configs WHERE connection_id = ?1 AND model_name = ?2",
            params![connection_id, model_name],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some(id) = existing {
        return Ok(id);
    }

    let id = Uuid::new_v4().to_string();
    sql.execute(
        "INSERT INTO model_configs (id, connection_id, model_name, context_length, gpu_offload, is_active) VALUES (?1, ?2, ?3, NULL, NULL, 0)",
        params![id, connection_id, model_name],
    )
    .map_err(|e| e.to_string())?;
    Ok(id)
}

#[derive(Debug, Serialize, Clone)]
pub struct DownloadableModel {
    #[serde(flatten)]
    pub info: CuratedModelInfo,
    pub fits_ram: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct DownloadableModelsResponse {
    pub ram_detected_gb: Option<f32>,
    pub models: Vec<DownloadableModel>,
}

#[derive(Debug, Serialize, Clone)]
struct ModelDownloadProgressEvent {
    connection_id: String,
    identifier: String,
    progress: PullProgress,
}

/// RAM detection failing (sysinfo returning 0 — rare/exotic environments)
/// never hides everything silently: every model is marked as fitting and
/// `ram_detected_gb` comes back `None` so the UI can show a warning instead.
#[tauri::command]
pub fn list_downloadable_models() -> DownloadableModelsResponse {
    let ram = system_info::total_ram_gb();
    let ram_known = ram > 0.0;
    let models = curated_models()
        .iter()
        .map(|m| {
            let info = CuratedModelInfo::from(m);
            let fits_ram = !ram_known || info.estimated_ram_gb <= ram;
            DownloadableModel { info, fits_ram }
        })
        .collect();
    DownloadableModelsResponse {
        ram_detected_gb: if ram_known { Some(ram) } else { None },
        models,
    }
}

#[tauri::command]
pub async fn list_installed_models(
    app: AppHandle,
    db: State<'_, DbState>,
    connection_id: String,
) -> Result<Vec<InstalledModel>, String> {
    let manager = embedded_commands::manager(&app);
    let conn = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        connections::list_connections(sql)?
            .into_iter()
            .find(|c| c.id == connection_id)
            .ok_or_else(|| "Conexão não encontrada".to_string())?
    };
    let client = manager.provider_for(&conn);
    client.list_installed_models().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pull_model(
    app: AppHandle,
    db: State<'_, DbState>,
    connection_id: String,
    identifier: String,
) -> Result<(), String> {
    let manager = embedded_commands::manager(&app);
    let conn = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        connections::list_connections(sql)?
            .into_iter()
            .find(|c| c.id == connection_id)
            .ok_or_else(|| "Conexão não encontrada".to_string())?
    };
    let client = manager.provider_for(&conn);

    let (tx, mut rx) = tokio::sync::mpsc::channel::<PullProgress>(32);
    let event_app = app.clone();
    let event_connection_id = connection_id.clone();
    let event_identifier = identifier.clone();
    let listener = tauri::async_runtime::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = event_app.emit(
                "model-download-progress",
                ModelDownloadProgressEvent {
                    connection_id: event_connection_id.clone(),
                    identifier: event_identifier.clone(),
                    progress,
                },
            );
        }
    });

    let result = client.pull_model(&identifier, tx).await;
    let _ = listener.await;
    result.map_err(|e| e.to_string())
}

/// Picking a model is a single action that also activates the connection it
/// belongs to (ACTIVE-05): two separate calls would leave a window where the
/// active model belongs to an inactive connection.
#[tauri::command]
pub fn set_active_model(
    db: State<DbState>,
    connection_id: String,
    model_name: String,
) -> Result<(), String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;

    let tx = sql.unchecked_transaction().map_err(|e| e.to_string())?;
    connections::apply_active_connection(&tx, &connection_id)?;
    let id = get_or_create_model_config(&tx, &connection_id, &model_name)?;
    tx.execute("UPDATE model_configs SET is_active = 0 WHERE is_active = 1", [])
        .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE model_configs SET is_active = 1 WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Clone)]
pub struct ActiveModel {
    pub connection_id: String,
    pub model_name: String,
    pub context_length: Option<u32>,
    pub gpu_offload: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ActivePair {
    pub connection: Option<Connection>,
    pub model: Option<ActiveModel>,
}

/// The chat needs "who answers now" as one answer, not two lists to cross
/// (ACTIVE-07). `model` can be `None` while `connection` is `Some` — the
/// user activated a connection but hasn't picked a model yet.
#[tauri::command]
pub fn get_active_pair(db: State<DbState>) -> Result<ActivePair, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;

    let connection = connections::active_connection(sql)?;
    let model = sql
        .query_row(
            "SELECT connection_id, model_name, context_length, gpu_offload FROM model_configs WHERE is_active = 1",
            [],
            |row| {
                Ok(ActiveModel {
                    connection_id: row.get(0)?,
                    model_name: row.get(1)?,
                    context_length: row.get(2)?,
                    gpu_offload: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    Ok(ActivePair { connection, model })
}

#[tauri::command]
pub async fn configure_model(
    app: AppHandle,
    db: State<'_, DbState>,
    connection_id: String,
    model_name: String,
    context_length: Option<u32>,
    gpu_offload: Option<String>,
) -> Result<ConfigApplied, String> {
    let manager = embedded_commands::manager(&app);
    let conn = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        connections::list_connections(sql)?
            .into_iter()
            .find(|c| c.id == connection_id)
            .ok_or_else(|| "Conexão não encontrada".to_string())?
    };

    let parsed_offload = gpu_offload.as_deref().map(GpuOffload::parse).transpose()?;

    let client = manager.provider_for(&conn);
    let applied = client
        .configure_model(&model_name, context_length, parsed_offload)
        .await
        .map_err(|e| e.to_string())?;

    {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        let id = get_or_create_model_config(sql, &connection_id, &model_name)?;
        sql.execute(
            "UPDATE model_configs SET context_length = ?1, gpu_offload = ?2 WHERE id = ?3",
            params![applied.context_length_applied, applied.gpu_offload_applied, id],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(applied)
}
