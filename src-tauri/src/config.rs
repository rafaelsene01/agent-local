use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

use crate::update::{self, InstallFlavor};

/// Folder created next to the executable when running the portable bundle.
/// A "portable" app that writes to %APPDATA% is not portable: it leaves the
/// machine dirty and does not survive being copied to another computer.
pub const PORTABLE_DATA_DIR: &str = "data";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub base_path: String,
    pub theme: String,
    pub language: String,
    pub onboarding_completed: bool,
    /// Update preferences (M8). Both `#[serde(default)]` so a config written by
    /// an older build keeps deserializing instead of resetting the wizard.
    #[serde(default = "default_true")]
    pub auto_update_check: bool,
    #[serde(default)]
    pub skipped_version: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            base_path: String::new(),
            theme: String::new(),
            language: String::new(),
            onboarding_completed: false,
            auto_update_check: true,
            skipped_version: None,
        }
    }
}

impl AppConfig {
    pub fn base_path_buf(&self) -> PathBuf {
        PathBuf::from(&self.base_path)
    }
}

/// Where the bootstrap pointer lives, given the install flavor.
///
/// Split out from [`bootstrap_file_path`] so it is testable without a real
/// `AppHandle` or a real executable location.
pub fn resolve_bootstrap_dir(
    flavor: InstallFlavor,
    app_dir: Option<&Path>,
    os_config_dir: &Path,
) -> Result<PathBuf, String> {
    match flavor {
        InstallFlavor::Portable => app_dir
            .map(|dir| dir.join(PORTABLE_DATA_DIR))
            .ok_or_else(|| "não foi possível localizar a pasta do aplicativo".to_string()),
        InstallFlavor::Installed => Ok(os_config_dir.to_path_buf()),
    }
}

/// Same split, for the folder the wizard offers by default.
pub fn resolve_default_base_path(
    flavor: InstallFlavor,
    app_dir: Option<&Path>,
    os_data_dir: &Path,
) -> Result<PathBuf, String> {
    resolve_bootstrap_dir(flavor, app_dir, os_data_dir)
}

/// Small bootstrap pointer file. It only stores *where* the user chose to put
/// their data (base_path) plus theme/language — the actual data (db, models,
/// documents, vectors) lives inside `base_path`, which the user controls. This
/// indirection is what lets the storage folder be reconfigurable without
/// knowing it in advance (AD-012).
///
/// Installed builds keep it in the OS-standard app config dir. Portable builds
/// keep it in `./data` next to the executable, so nothing is left behind on the
/// host machine (AD-034).
fn bootstrap_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let os_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("failed to resolve app config dir: {e}"))?;

    let dir = resolve_bootstrap_dir(update::flavor(), update::app_dir().as_deref(), &os_dir)?;
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
    let os_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

    let dir = resolve_default_base_path(update::flavor(), update::app_dir().as_deref(), &os_dir)?;
    Ok(dir.to_string_lossy().to_string())
}

/// `runtime` holds the downloaded llama.cpp binary: it is user data under the
/// chosen base path (AD-008), not a temp file, so it survives app updates.
const SUBDIRS: [&str; 5] = ["models", "documents", "vectors", "chats", "runtime"];

pub fn ensure_folder_structure(base_path: &Path) -> Result<(), String> {
    fs::create_dir_all(base_path)
        .map_err(|e| format!("não foi possível criar a pasta '{}': {e}", base_path.display()))?;

    // Fail fast on read-only / no-permission folders instead of silently
    // succeeding and breaking later on the first write. This is also what
    // catches a portable copy dropped on a write-protected drive.
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

/// What the app knows about the storage folder at boot.
///
/// `configured` without `ready` is the case this type exists for: the pointer
/// says onboarding is done, but the folder it points at is gone (external drive
/// unplugged, folder moved or deleted between sessions). Before this, boot only
/// logged the failure and left the database unopened, so the app came up
/// looking normal and every single command failed with "Nenhuma pasta de
/// armazenamento configurada ainda". The spec asks for a warning and the wizard.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct StorageStatus {
    pub configured: bool,
    pub ready: bool,
    pub base_path: String,
}

/// The decision itself, split from the I/O so it can be tested.
///
/// `db_open` matters on its own: a folder that exists but whose `localmind.db`
/// could not be opened (corrupted file, permissions) is just as unusable, and
/// the wizard recovers both — `complete_onboarding` recreates the structure and
/// reopens the connection.
pub fn evaluate_storage(config: Option<&AppConfig>, dir_exists: bool, db_open: bool) -> StorageStatus {
    match config {
        Some(cfg) if cfg.onboarding_completed => StorageStatus {
            configured: true,
            ready: dir_exists && db_open,
            base_path: cfg.base_path.clone(),
        },
        _ => StorageStatus {
            configured: false,
            ready: false,
            base_path: String::new(),
        },
    }
}

pub fn storage_status(app: &AppHandle, db_open: bool) -> Result<StorageStatus, String> {
    let cfg = load_config(app)?;
    let dir_exists = cfg
        .as_ref()
        .map(|c| c.base_path_buf().is_dir())
        .unwrap_or(false);
    Ok(evaluate_storage(cfg.as_ref(), dir_exists, db_open))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_builds_keep_using_the_os_folders() {
        let os_dir = Path::new("/os/config/com.localmind.app");
        let app_dir = Path::new("/opt/localmind");

        // No regression on AD-012: the pointer stays where it has always been.
        assert_eq!(
            resolve_bootstrap_dir(InstallFlavor::Installed, Some(app_dir), os_dir).unwrap(),
            os_dir
        );
    }

    #[test]
    fn portable_builds_stay_next_to_the_executable() {
        let os_dir = Path::new("/os/config/com.localmind.app");
        let app_dir = Path::new("/media/usb/LocalMind");

        assert_eq!(
            resolve_bootstrap_dir(InstallFlavor::Portable, Some(app_dir), os_dir).unwrap(),
            app_dir.join(PORTABLE_DATA_DIR)
        );
        assert_eq!(
            resolve_default_base_path(InstallFlavor::Portable, Some(app_dir), os_dir).unwrap(),
            app_dir.join(PORTABLE_DATA_DIR)
        );
    }

    #[test]
    fn portable_without_a_known_app_dir_is_an_error_not_a_silent_fallback() {
        // Falling back to %APPDATA% here would quietly break the one promise
        // the portable bundle makes.
        let os_dir = Path::new("/os/config");
        assert!(resolve_bootstrap_dir(InstallFlavor::Portable, None, os_dir).is_err());
    }

    fn completed_config(base_path: &str) -> AppConfig {
        AppConfig {
            base_path: base_path.to_string(),
            onboarding_completed: true,
            ..Default::default()
        }
    }

    #[test]
    fn a_configured_and_present_folder_is_ready() {
        let cfg = completed_config("/data/localmind");
        let status = evaluate_storage(Some(&cfg), true, true);
        assert!(status.configured && status.ready);
        assert_eq!(status.base_path, "/data/localmind");
    }

    #[test]
    fn a_folder_that_vanished_is_configured_but_not_ready() {
        // The whole point: the app must not come up "ready" pointing at a
        // folder that is not there. The path rides along so the warning can
        // name it.
        let cfg = completed_config("E:/localmind");
        let status = evaluate_storage(Some(&cfg), false, false);
        assert!(status.configured);
        assert!(!status.ready);
        assert_eq!(status.base_path, "E:/localmind");
    }

    #[test]
    fn a_folder_that_exists_but_whose_database_failed_to_open_is_not_ready() {
        let cfg = completed_config("/data/localmind");
        assert!(!evaluate_storage(Some(&cfg), true, false).ready);
    }

    #[test]
    fn no_config_and_unfinished_onboarding_both_mean_not_configured() {
        assert!(!evaluate_storage(None, true, true).configured);

        let mut cfg = completed_config("/data/localmind");
        cfg.onboarding_completed = false;
        let status = evaluate_storage(Some(&cfg), true, true);
        assert!(!status.configured);
        // Nothing to warn about yet, so no path is exposed.
        assert_eq!(status.base_path, "");
    }

    #[test]
    fn automatic_update_check_defaults_to_on() {
        assert!(AppConfig::default().auto_update_check);
        assert!(AppConfig::default().skipped_version.is_none());
    }

    #[test]
    fn a_config_written_before_m8_still_deserializes() {
        let legacy = r#"{
            "base_path": "D:/dados",
            "theme": "dark",
            "language": "pt",
            "onboarding_completed": true
        }"#;
        let cfg: AppConfig = serde_json::from_str(legacy).unwrap();
        assert_eq!(cfg.base_path, "D:/dados");
        assert!(cfg.onboarding_completed);
        assert!(cfg.auto_update_check, "missing field must default to on");
        assert!(cfg.skipped_version.is_none());
    }

    #[test]
    fn the_update_preferences_round_trip() {
        let mut cfg = AppConfig {
            base_path: "D:/dados".into(),
            theme: "dark".into(),
            language: "pt".into(),
            onboarding_completed: true,
            ..Default::default()
        };
        cfg.auto_update_check = false;
        cfg.skipped_version = Some("1.2.3".into());

        let round: AppConfig = serde_json::from_str(&serde_json::to_string(&cfg).unwrap()).unwrap();
        assert!(!round.auto_update_check);
        assert_eq!(round.skipped_version.as_deref(), Some("1.2.3"));
    }
}
