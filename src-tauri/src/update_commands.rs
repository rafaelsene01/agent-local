//! Update commands.
//!
//! One surface for the frontend, two implementations behind it: the official
//! plugin for installed builds, `update::portable` for the portable bundle. The
//! UI never learns which one it got beyond a badge.

use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

use crate::config;
use crate::update::{self, manifest, portable, InstallFlavor, UpdateInfo};

#[derive(Debug, Serialize)]
pub struct UpdateSettings {
    pub current_version: String,
    pub auto_check: bool,
    pub flavor: InstallFlavor,
    pub skipped_version: Option<String>,
}

/// Reads `plugins.updater` out of the embedded `tauri.conf.json`.
///
/// Going through the serialized config rather than a typed accessor keeps this
/// working regardless of the plugin-config struct's shape, and keeps the public
/// key and the endpoint defined in exactly one place.
fn updater_config(app: &AppHandle) -> Option<serde_json::Value> {
    serde_json::to_value(app.config())
        .ok()?
        .get("plugins")?
        .get("updater")
        .cloned()
}

fn updater_pubkey(app: &AppHandle) -> Result<String, String> {
    updater_config(app)
        .as_ref()
        .and_then(|cfg| cfg.get("pubkey"))
        .and_then(|key| key.as_str())
        .filter(|key| !key.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            "chave pública de atualização não configurada (plugins.updater.pubkey)".to_string()
        })
}

fn updater_endpoint(app: &AppHandle) -> Result<String, String> {
    updater_config(app)
        .as_ref()
        .and_then(|cfg| cfg.get("endpoints"))
        .and_then(|list| list.as_array())
        .and_then(|list| list.first())
        .and_then(|url| url.as_str())
        .map(str::to_string)
        .ok_or_else(|| "endpoint de atualização não configurado".to_string())
}

fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

#[tauri::command]
pub async fn check_for_update(app: AppHandle) -> Result<Option<UpdateInfo>, String> {
    let current = current_version(&app);
    let skipped = config::load_config(&app)
        .ok()
        .flatten()
        .and_then(|cfg| cfg.skipped_version);

    let found = match update::flavor() {
        InstallFlavor::Installed => check_installed(&app, &current).await?,
        InstallFlavor::Portable => check_portable(&app, &current).await?,
    };

    // "Skip this version" only silences that exact version; a later one still
    // shows up.
    Ok(found.filter(|info| skipped.as_deref() != Some(info.version.as_str())))
}

async fn check_installed(app: &AppHandle, current: &str) -> Result<Option<UpdateInfo>, String> {
    let updater = app
        .updater()
        .map_err(|e| format!("não foi possível consultar atualizações: {e}"))?;

    let found = updater
        .check()
        .await
        .map_err(|e| format!("não foi possível consultar atualizações: {e}"))?;

    Ok(found.map(|found| UpdateInfo {
        version: found.version.clone(),
        current_version: current.to_string(),
        notes: found.body.clone(),
        pub_date: found.date.map(|date| date.to_string()),
        flavor: InstallFlavor::Installed,
    }))
}

async fn check_portable(app: &AppHandle, current: &str) -> Result<Option<UpdateInfo>, String> {
    let endpoint = updater_endpoint(app)?;
    let found = manifest::fetch(&endpoint).await?;

    if !update::is_newer(&found.version, current) {
        return Ok(None);
    }

    // No portable artifact for this platform is "nothing for you", not an error.
    let Some(key) = manifest::platform_key(InstallFlavor::Portable) else {
        return Ok(None);
    };
    if manifest::select(&found, key).is_none() {
        return Ok(None);
    }

    Ok(Some(UpdateInfo {
        version: found.version,
        current_version: current.to_string(),
        notes: found.notes,
        pub_date: found.pub_date,
        flavor: InstallFlavor::Portable,
    }))
}

#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    match update::flavor() {
        InstallFlavor::Installed => install_installed(&app).await,
        InstallFlavor::Portable => install_portable(&app).await,
    }
}

async fn install_installed(app: &AppHandle) -> Result<(), String> {
    let updater = app
        .updater()
        .map_err(|e| format!("não foi possível consultar atualizações: {e}"))?;

    let found = updater
        .check()
        .await
        .map_err(|e| format!("não foi possível consultar atualizações: {e}"))?
        .ok_or_else(|| "nenhuma atualização disponível".to_string())?;

    // on_chunk reports the chunk size, not the running total.
    let downloaded = Arc::new(AtomicU64::new(0));
    let handle = app.clone();
    let counter = downloaded.clone();

    found
        .download_and_install(
            move |chunk, total| {
                let so_far = counter.fetch_add(chunk as u64, Ordering::Relaxed) + chunk as u64;
                let _ = handle.emit(
                    portable::UPDATE_PROGRESS_EVENT,
                    portable::DownloadProgress {
                        downloaded: so_far,
                        total,
                    },
                );
            },
            || {},
        )
        .await
        .map_err(|e| format!("falha ao instalar a atualização: {e}"))?;

    app.restart();
}

async fn install_portable(app: &AppHandle) -> Result<(), String> {
    let app_dir =
        update::app_dir().ok_or_else(|| "não foi possível localizar a pasta do aplicativo".to_string())?;
    let pubkey = updater_pubkey(app)?;
    let endpoint = updater_endpoint(app)?;

    let found = manifest::fetch(&endpoint).await?;
    if !update::is_newer(&found.version, &current_version(app)) {
        return Err("nenhuma atualização disponível".to_string());
    }

    let key = manifest::platform_key(InstallFlavor::Portable)
        .ok_or_else(|| "não há pacote portátil para esta plataforma".to_string())?;
    let entry = manifest::select(&found, key)
        .ok_or_else(|| "esta versão não publicou um pacote portátil".to_string())?
        .clone();

    let exe = portable::install(app, &app_dir, &entry, &pubkey).await?;

    // Not `app.restart()`: it relaunches `current_exe()`, which by now points at
    // the renamed `.old` file. Spawn the new path explicitly, then quit.
    std::process::Command::new(&exe)
        .spawn()
        .map_err(|e| format!("atualização aplicada, mas o app não reiniciou sozinho: {e}"))?;

    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn skip_update_version(app: AppHandle, version: String) -> Result<(), String> {
    let mut cfg =
        config::load_config(&app)?.ok_or_else(|| "Configuração não encontrada".to_string())?;
    cfg.skipped_version = Some(version);
    config::save_config(&app, &cfg)
}

#[tauri::command]
pub fn get_update_settings(app: AppHandle) -> Result<UpdateSettings, String> {
    let cfg = config::load_config(&app)?;
    Ok(UpdateSettings {
        current_version: current_version(&app),
        // Before onboarding there is no config yet; the default is "on", the
        // same value the wizard will persist.
        auto_check: cfg.as_ref().map(|c| c.auto_update_check).unwrap_or(true),
        flavor: update::flavor(),
        skipped_version: cfg.and_then(|c| c.skipped_version),
    })
}

#[tauri::command]
pub fn set_auto_update_check(app: AppHandle, enabled: bool) -> Result<(), String> {
    let mut cfg =
        config::load_config(&app)?.ok_or_else(|| "Configuração não encontrada".to_string())?;
    cfg.auto_update_check = enabled;
    config::save_config(&app, &cfg)
}

/// Called at boot: removes the executable left behind by a previous portable
/// update. Silent by design — a leftover file must never block startup.
pub fn cleanup_after_update() {
    if let Some(dir) = update::app_dir() {
        portable::cleanup_old_files(&dir);
    }
}
