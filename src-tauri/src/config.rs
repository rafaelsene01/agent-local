use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub base_path: String,
    pub theme: String,
    pub language: String,
    pub onboarding_completed: bool,
}

impl AppConfig {
    pub fn base_path_buf(&self) -> PathBuf {
        PathBuf::from(&self.base_path)
    }
}

/// Small bootstrap pointer file that lives in the OS-standard app config dir.
/// It only stores *where* the user chose to put their data (base_path) plus
/// theme/language — the actual data (db, models, documents, vectors) lives
/// inside `base_path`, which the user controls. This indirection is what lets
/// the storage folder be reconfigurable without knowing it in advance.
fn bootstrap_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("failed to resolve app config dir: {e}"))?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("config.json"))
}

pub fn load_config(app: &AppHandle) -> Result<Option<AppConfig>, String> {
    let path = bootstrap_file_path(app)?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    match serde_json::from_str::<AppConfig>(&raw) {
        Ok(cfg) => Ok(Some(cfg)),
        Err(_) => Ok(None), // corrupted config -> fall back to defaults / re-run wizard
    }
}

pub fn save_config(app: &AppHandle, config: &AppConfig) -> Result<(), String> {
    let path = bootstrap_file_path(app)?;
    let raw = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, raw).map_err(|e| e.to_string())
}

pub fn default_base_path(app: &AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;
    Ok(dir.to_string_lossy().to_string())
}

const SUBDIRS: [&str; 4] = ["models", "documents", "vectors", "chats"];

pub fn ensure_folder_structure(base_path: &Path) -> Result<(), String> {
    fs::create_dir_all(base_path)
        .map_err(|e| format!("não foi possível criar a pasta '{}': {e}", base_path.display()))?;

    // Fail fast on read-only / no-permission folders instead of silently
    // succeeding and breaking later on the first write.
    let probe = base_path.join(".localmind-write-test");
    fs::write(&probe, b"ok").map_err(|e| format!("pasta sem permissão de escrita: {e}"))?;
    let _ = fs::remove_file(&probe);

    for sub in SUBDIRS {
        fs::create_dir_all(base_path.join(sub)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn db_path(base_path: &Path) -> PathBuf {
    base_path.join("localmind.db")
}
