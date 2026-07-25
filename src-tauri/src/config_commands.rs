use crate::config::{self, AppConfig};
use crate::db::{self, DbState};
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub fn get_app_config(app: AppHandle) -> Result<Option<AppConfig>, String> {
    config::load_config(&app)
}

#[tauri::command]
pub fn get_default_base_path(app: AppHandle) -> Result<String, String> {
    config::default_base_path(&app)
}

#[tauri::command]
pub fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    match app.dialog().file().blocking_pick_folder() {
        Some(file_path) => {
            let path = file_path.into_path().map_err(|e| e.to_string())?;
            Ok(Some(path.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub fn complete_onboarding(
    app: AppHandle,
    db: State<DbState>,
    base_path: String,
    theme: String,
    language: String,
) -> Result<AppConfig, String> {
    let base = PathBuf::from(&base_path);
    config::ensure_folder_structure(&base)?;

    let conn = db::open(&config::db_path(&base))?;
    {
        let mut guard = db.0.lock().map_err(|e| e.to_string())?;
        *guard = Some(conn);
    }

    let cfg = AppConfig {
        base_path,
        theme,
        language,
        onboarding_completed: true,
    };
    config::save_config(&app, &cfg)?;
    Ok(cfg)
}

#[tauri::command]
pub fn update_theme(app: AppHandle, theme: String) -> Result<AppConfig, String> {
    let mut cfg = config::load_config(&app)?.ok_or_else(|| "Configuração não encontrada".to_string())?;
    cfg.theme = theme;
    config::save_config(&app, &cfg)?;
    Ok(cfg)
}

#[tauri::command]
pub fn update_language(app: AppHandle, language: String) -> Result<AppConfig, String> {
    let mut cfg = config::load_config(&app)?.ok_or_else(|| "Configuração não encontrada".to_string())?;
    cfg.language = language;
    config::save_config(&app, &cfg)?;
    Ok(cfg)
}

/// Moves only `localmind.db` to the new folder (chats/messages). Documents,
/// models and vectors already on disk under the old base_path are NOT moved
/// automatically in v1 — see .specs/features/settings-storage-i18n/spec.md
/// edge cases.
#[tauri::command]
pub fn update_base_path(
    app: AppHandle,
    db: State<DbState>,
    new_base_path: String,
) -> Result<AppConfig, String> {
    let mut cfg = config::load_config(&app)?.ok_or_else(|| "Configuração não encontrada".to_string())?;
    let old_base = cfg.base_path_buf();
    let new_base = PathBuf::from(&new_base_path);

    config::ensure_folder_structure(&new_base)?;

    let old_db_file = config::db_path(&old_base);
    let new_db_file = config::db_path(&new_base);

    {
        // Drop the current connection first so the old db file isn't locked.
        let mut guard = db.0.lock().map_err(|e| e.to_string())?;
        *guard = None;
    }

    if old_db_file.exists() && old_db_file != new_db_file {
        std::fs::rename(&old_db_file, &new_db_file)
            .or_else(|_| {
                std::fs::copy(&old_db_file, &new_db_file)
                    .map(|_| ())
                    .and_then(|_| std::fs::remove_file(&old_db_file))
            })
            .map_err(|e| format!("falha ao mover o banco de dados: {e}"))?;
    }

    let conn = db::open(&new_db_file)?;
    {
        let mut guard = db.0.lock().map_err(|e| e.to_string())?;
        *guard = Some(conn);
    }

    cfg.base_path = new_base_path;
    config::save_config(&app, &cfg)?;
    Ok(cfg)
}
