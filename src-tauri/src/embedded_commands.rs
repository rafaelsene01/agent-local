use crate::config;
use crate::db::{require_conn, DbState};
use crate::providers::{PullProgress, PullStatus};
use crate::runtime::detect::{probe_devices, DeviceProbe};
use crate::runtime::process::{free_port, spawn, SidecarConfig, SidecarState};
use crate::runtime::store::{self, EmbeddedRuntimeRow};
use crate::runtime::{detect, download, model, release, Backend, RuntimeError, TargetOs};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddedSetupStage {
    Unsupported,
    NotInstalled,
    DownloadingBinary,
    DownloadingModel,
    Ready,
    Running,
    Error,
}

#[derive(Debug, Serialize, Clone)]
pub struct EmbeddedRuntimeStatus {
    pub stage: EmbeddedSetupStage,
    pub release_tag: Option<String>,
    pub backend: Option<String>,
    pub port: Option<u16>,
    pub model_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct EmbeddedSetupProgress {
    stage: EmbeddedSetupStage,
    progress: Option<PullProgress>,
    message: Option<String>,
}

fn emit_stage(app: &AppHandle, stage: EmbeddedSetupStage, message: Option<String>) {
    let _ = app.emit(
        "embedded-setup-progress",
        EmbeddedSetupProgress {
            stage,
            progress: None,
            message,
        },
    );
}

/// Forwards byte-level progress from a download channel to the frontend under
/// the current stage, reusing the same `PullProgress` shape the model
/// download UI already renders.
fn forward_progress(
    app: &AppHandle,
    stage: EmbeddedSetupStage,
) -> tokio::sync::mpsc::Sender<PullProgress> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<PullProgress>(32);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = app.emit(
                "embedded-setup-progress",
                EmbeddedSetupProgress {
                    stage: stage.clone(),
                    progress: Some(progress),
                    message: None,
                },
            );
        }
    });
    tx
}

fn base_path(app: &AppHandle) -> Result<PathBuf, String> {
    config::load_config(app)?
        .map(|cfg| cfg.base_path_buf())
        .ok_or_else(|| "Nenhuma pasta de armazenamento configurada ainda".to_string())
}

pub fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(base_path(app)?.join("models"))
}

fn runtime_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(base_path(app)?.join("runtime"))
}

/// The archive lays the binaries out differently per platform/build, so the
/// extracted tree is searched instead of assuming a fixed path.
fn find_server_binary(dir: &Path) -> Option<PathBuf> {
    let target = if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    };
    let entries = std::fs::read_dir(dir).ok()?;
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else if path.file_name().is_some_and(|n| n == target) {
            return Some(path);
        }
    }
    dirs.iter().find_map(|d| find_server_binary(d))
}

async fn download_and_extract_backend(
    app: &AppHandle,
    tag_assets: &[release::Asset],
    os: TargetOs,
    backend: Backend,
    runtime_dir: &Path,
) -> Result<PathBuf, String> {
    let asset = release::pick_asset(tag_assets, os, backend).ok_or_else(|| {
        RuntimeError::AssetNotFound(release::asset_suffix(os, backend).to_string()).to_string()
    })?;

    let archive = runtime_dir.join(&asset.name);
    let progress = forward_progress(app, EmbeddedSetupStage::DownloadingBinary);
    download::download_with_progress(&asset.browser_download_url, &archive, progress)
        .await
        .map_err(|e| e.to_string())?;

    let dest = runtime_dir.join(backend.as_str());
    let _ = std::fs::remove_dir_all(&dest);
    download::extract(&archive, &dest).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&archive);

    find_server_binary(&dest).ok_or_else(|| {
        format!(
            "o arquivo baixado não contém o llama-server (procurado em {})",
            dest.display()
        )
    })
}

/// Downloads the Vulkan build first and only falls back to the CPU build when
/// the Vulkan binary cannot even run: the Vulkan build works fine on CPU with
/// `-ngl 0`, so the common path is a single download (AD-022).
#[tauri::command]
pub async fn setup_embedded_runtime(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<EmbeddedRuntimeStatus, String> {
    let Some(os) = TargetOs::current() else {
        return Err(RuntimeError::UnsupportedPlatform.to_string());
    };
    let runtime_dir = runtime_dir(&app)?;
    let models_dir = models_dir(&app)?;
    std::fs::create_dir_all(&runtime_dir).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

    emit_stage(&app, EmbeddedSetupStage::DownloadingBinary, None);
    let latest = release::resolve_latest().await.map_err(|e| e.to_string())?;

    let mut backend = Backend::Vulkan;
    let mut binary =
        download_and_extract_backend(&app, &latest.assets, os, backend, &runtime_dir).await?;

    let mut gpu_layers = 0;
    match probe_devices(&binary) {
        DeviceProbe::GpuAvailable(name) => {
            gpu_layers = -1;
            emit_stage(
                &app,
                EmbeddedSetupStage::DownloadingBinary,
                Some(format!("GPU detectada: {name}")),
            );
        }
        DeviceProbe::CpuOnly => {
            emit_stage(
                &app,
                EmbeddedSetupStage::DownloadingBinary,
                Some("Nenhuma GPU compatível encontrada — usando CPU".to_string()),
            );
        }
        DeviceProbe::BinaryFailed(reason) => {
            // The Vulkan binary can't even start here (no loader): a
            // different asset is needed, not just different flags.
            emit_stage(
                &app,
                EmbeddedSetupStage::DownloadingBinary,
                Some(format!("Build Vulkan não executou ({reason}) — baixando a versão CPU")),
            );
            backend = Backend::Cpu;
            binary =
                download_and_extract_backend(&app, &latest.assets, os, backend, &runtime_dir).await?;
            if let DeviceProbe::BinaryFailed(reason) = detect::probe_devices(&binary) {
                return Err(format!("o llama-server não executa nesta máquina: {reason}"));
            }
        }
    }

    emit_stage(&app, EmbeddedSetupStage::DownloadingModel, None);
    let progress = forward_progress(&app, EmbeddedSetupStage::DownloadingModel);
    let model_path = model::download_default_model(&models_dir, progress)
        .await
        .map_err(|e| e.to_string())?;

    let row = EmbeddedRuntimeRow {
        release_tag: Some(latest.tag_name),
        backend: Some(backend.as_str().to_string()),
        binary_path: Some(binary.to_string_lossy().to_string()),
        model_path: Some(model_path.to_string_lossy().to_string()),
        context_length: None,
        gpu_layers: Some(gpu_layers),
    };
    {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        store::save(sql, &row)?;
    }

    let _ = app.emit(
        "embedded-setup-progress",
        EmbeddedSetupProgress {
            stage: EmbeddedSetupStage::Ready,
            progress: Some(PullProgress {
                status: PullStatus::Success,
                downloaded_bytes: None,
                total_bytes: None,
                message: None,
            }),
            message: None,
        },
    );

    Ok(status_from(&row, None, EmbeddedSetupStage::Ready))
}

/// Shared by the command and by boot autostart, which has no `State` handle.
pub async fn start_sidecar_from_row(
    app: &AppHandle,
    row: &EmbeddedRuntimeRow,
) -> Result<u16, String> {
    let (Some(binary), Some(model_path)) = (&row.binary_path, &row.model_path) else {
        return Err("o runtime embutido ainda não foi instalado".to_string());
    };

    if let Some(existing) = app.state::<SidecarState>().0.lock().ok().and_then(|g| {
        g.as_ref().map(|s| s.port)
    }) {
        return Ok(existing);
    }

    let cfg = SidecarConfig {
        binary: PathBuf::from(binary),
        model: PathBuf::from(model_path),
        port: free_port().map_err(|e| e.to_string())?,
        context_length: row.context_length,
        gpu_layers: row.gpu_layers.unwrap_or(0),
    };
    let port = cfg.port;
    let sidecar = spawn(cfg).await.map_err(|e| e.to_string())?;

    let state = app.state::<SidecarState>();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    *guard = Some(sidecar);
    Ok(port)
}

#[tauri::command]
pub async fn start_embedded_runtime(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<EmbeddedRuntimeStatus, String> {
    let row = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        store::load(sql)?
    };
    let port = start_sidecar_from_row(&app, &row).await?;
    Ok(status_from(&row, Some(port), EmbeddedSetupStage::Running))
}

#[tauri::command]
pub fn stop_embedded_runtime(app: AppHandle) -> Result<(), String> {
    let state = app.state::<SidecarState>();
    let mut guard = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(mut sidecar) = guard.take() {
        sidecar.kill();
    }
    Ok(())
}

pub fn running_port(app: &AppHandle) -> Option<u16> {
    app.state::<SidecarState>()
        .0
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|s| s.port))
}

fn status_from(
    row: &EmbeddedRuntimeRow,
    port: Option<u16>,
    stage: EmbeddedSetupStage,
) -> EmbeddedRuntimeStatus {
    EmbeddedRuntimeStatus {
        stage,
        release_tag: row.release_tag.clone(),
        backend: row.backend.clone(),
        port,
        model_name: row.model_path.as_ref().and_then(|p| {
            Path::new(p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
        }),
        message: None,
    }
}

#[tauri::command]
pub fn embedded_runtime_status(
    app: AppHandle,
    db: State<DbState>,
) -> Result<EmbeddedRuntimeStatus, String> {
    if TargetOs::current().is_none() {
        return Ok(EmbeddedRuntimeStatus {
            stage: EmbeddedSetupStage::Unsupported,
            release_tag: None,
            backend: None,
            port: None,
            model_name: None,
            message: Some(RuntimeError::UnsupportedPlatform.to_string()),
        });
    }

    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;
    let row = store::load(sql)?;
    let port = running_port(&app);

    let stage = if port.is_some() {
        EmbeddedSetupStage::Running
    } else if row.is_ready() {
        EmbeddedSetupStage::Ready
    } else {
        EmbeddedSetupStage::NotInstalled
    };

    Ok(status_from(&row, port, stage))
}

/// EMBED-13: any direct `.gguf` link, downloaded into the same models folder
/// the sidecar already reads from.
#[tauri::command]
pub async fn download_embedded_model(app: AppHandle, url: String) -> Result<(), String> {
    let models_dir = models_dir(&app)?;
    let progress = forward_progress(&app, EmbeddedSetupStage::DownloadingModel);
    model::download_model_from_url(&url, &models_dir, progress)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}
