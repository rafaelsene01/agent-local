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
