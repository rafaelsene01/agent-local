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

    // EMBED-04 AC4: with binary and model in place the sidecar comes up on its
    // own, so the connection reports "available" without another click. A
    // failure here still leaves a usable installed state to start manually.
    match start_sidecar_from_row(&app, &row).await {
        Ok(port) => Ok(status_from(&row, Some(port), EmbeddedSetupStage::Running)),
        Err(e) => {
            let mut status = status_from(&row, None, EmbeddedSetupStage::Ready);
            status.message = Some(e);
            Ok(status)
        }
    }
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
        // Before onboarding there is no folder to write into, and the sidecar
        // starts without a log rather than not at all.
        base_path: crate::config::load_config(app)
            .ok()
            .flatten()
            .map(|cfg| cfg.base_path_buf()),
    };
    let port = cfg.port;
    let sidecar = spawn(cfg, &app.state::<crate::runtime::job::JobState>())
        .await
        .map_err(|e| e.to_string())?;

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

/// Context length and GPU offload are `llama-server` startup flags, so
/// applying them means rewriting the persisted row and restarting the process
/// when it is up — otherwise the setting would sit in the database and never
/// reach the server (EMBED-12).
pub async fn apply_runtime_config(
    app: &AppHandle,
    db: &DbState,
    context_length: Option<u32>,
    gpu_layers: Option<i32>,
) -> Result<(), String> {
    let row = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        let mut row = store::load(sql)?;
        row.context_length = context_length;
        if let Some(layers) = gpu_layers {
            row.gpu_layers = Some(layers);
        }
        store::save(sql, &row)?;
        row
    };

    if running_port(app).is_some() {
        stop_embedded_runtime(app.clone())?;
        start_sidecar_from_row(app, &row).await?;
    }
    Ok(())
}

/// The model is a `-m` startup flag, so switching it means rewriting the
/// persisted row and restarting the process — the same shape as
/// `apply_runtime_config`, and the reason picking a model for the embedded
/// runtime can't be a pure database write (EMBED-05).
pub async fn apply_active_model(
    app: &AppHandle,
    db: &DbState,
    model_name: &str,
) -> Result<(), String> {
    let path = models_dir(app)?.join(model_name);
    if !path.exists() {
        return Err(format!(
            "o arquivo {} não está na pasta de modelos",
            path.display()
        ));
    }
    let wanted = path.to_string_lossy().to_string();

    let (row, changed) = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        let mut row = store::load(sql)?;
        let changed = row.model_path.as_deref() != Some(wanted.as_str());
        if changed {
            row.model_path = Some(wanted);
            store::save(sql, &row)?;
        }
        (row, changed)
    };

    if changed && running_port(app).is_some() {
        stop_embedded_runtime(app.clone())?;
        start_sidecar_from_row(app, &row).await?;
    }
    Ok(())
}

/// Every caller that talks to the sidecar builds its client through here, so
/// it always resolves to the port the process actually picked instead of a
/// placeholder URL. A stopped runtime yields a client whose calls report
/// `Unavailable` — which is a state, not an error to handle here.
pub fn client(app: &AppHandle) -> crate::providers::llama_server::LlamaServerClient {
    crate::providers::llama_server::LlamaServerClient::new(
        running_port(app),
        models_dir(app).unwrap_or_default(),
    )
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

// ---------------------------------------------------------------------------
// Model surface (SELF-01/SELF-02)
//
// These replace `model_commands.rs`. Every one of them lost its
// `connection_id`: there is one runtime, so there is nothing to disambiguate.
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Serialize, Clone)]
pub struct DownloadableModel {
    #[serde(flatten)]
    pub info: crate::models::catalog::CuratedModelInfo,
    pub fits_ram: bool,
}

#[derive(Debug, serde::Serialize, Clone)]
pub struct DownloadableModelsResponse {
    pub ram_detected_gb: Option<f32>,
    pub models: Vec<DownloadableModel>,
}

/// RAM detection failing (sysinfo returning 0 — rare/exotic environments) never
/// hides everything silently: every model is marked as fitting and
/// `ram_detected_gb` comes back `None` so the UI can warn instead.
#[tauri::command]
pub fn list_downloadable_models() -> DownloadableModelsResponse {
    use crate::models::catalog::{curated_models, CuratedModelInfo};

    let ram = crate::system_info::total_ram_gb();
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

/// The GGUF files on disk, which is also the list of what can be made active.
#[tauri::command]
pub fn list_installed_models(app: AppHandle) -> Vec<crate::providers::InstalledModel> {
    client(&app).list_installed_models()
}

#[tauri::command]
pub async fn model_limits(
    app: AppHandle,
    model: String,
) -> Result<crate::providers::ModelLimits, String> {
    client(&app)
        .model_limits(&model)
        .await
        .map_err(|e| e.to_string())
}

/// What the chat will use. `None` means "nothing chosen, or the file is gone" —
/// the two cases the user resolves the same way.
#[tauri::command]
pub fn get_active_model(db: State<DbState>) -> Result<Option<store::ActiveModel>, String> {
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = crate::db::require_conn(&guard)?;
    store::active_model(sql)
}

/// Points the runtime at another file and restarts it, *then* records the
/// choice. The order matters: a failure to restart must not leave a model
/// marked active that the runtime is not actually serving.
#[tauri::command]
pub async fn set_active_model(
    app: AppHandle,
    db: State<'_, DbState>,
    model_name: String,
) -> Result<(), String> {
    apply_active_model(&app, &db, &model_name).await
}

/// Context length and GPU offload are start-up flags, so applying them restarts
/// the sidecar — `requires_reload` in the old API said so, and the behaviour is
/// unchanged (EMBED-12).
#[tauri::command]
pub async fn configure_model(
    app: AppHandle,
    db: State<'_, DbState>,
    context_length: Option<u32>,
    gpu_offload: Option<String>,
) -> Result<(), String> {
    let gpu_layers = match gpu_offload.as_deref() {
        Some(raw) => Some(crate::providers::gpu_layers_for(
            &crate::providers::GpuOffload::parse(raw)?,
        )),
        None => None,
    };
    apply_runtime_config(&app, &db, context_length, gpu_layers).await
}
