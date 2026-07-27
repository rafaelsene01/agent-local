// SPEC: self-contained-runtime (SELF-01, SELF-02, SELF-07, SELF-08)

use crate::config;
use crate::db::{require_conn, DbState};
use crate::providers::{PullProgress, PullStatus};
use crate::runtime::detect::{probe_devices, DeviceProbe};
use crate::runtime::process::{free_port, spawn, SidecarConfig, SidecarState};
use crate::runtime::store::{self, EmbeddedRuntimeRow};
use crate::runtime::{bundled, detect, model, Backend, RuntimeError, TargetOs};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager, State};

/// The states the runtime card renders. There is no `DownloadingModel` stage:
/// downloading a GGUF is no longer part of preparing the engine, so it reports
/// on its own channel (`model-download-progress`) and a machine with a `.gguf`
/// already in the models folder never needs the network (SELF-11).
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStage {
    Unsupported,
    NotPrepared,
    Preparing,
    NoModel,
    Ready,
    Running,
}

#[derive(Debug, Serialize, Clone)]
pub struct RuntimeStatus {
    pub stage: RuntimeStage,
    pub release_tag: Option<String>,
    pub backend: Option<String>,
    pub port: Option<u16>,
    pub model_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
struct RuntimeProgress {
    stage: RuntimeStage,
    progress: Option<PullProgress>,
    message: Option<String>,
}

fn emit_stage(app: &AppHandle, stage: RuntimeStage, message: Option<String>) {
    let _ = app.emit(
        "runtime-progress",
        RuntimeProgress {
            stage,
            progress: None,
            message,
        },
    );
}

fn base_path(app: &AppHandle) -> Result<PathBuf, String> {
    config::load_config(app)?
        .map(|cfg| cfg.base_path_buf())
        .ok_or_else(|| "Nenhuma pasta de armazenamento configurada ainda".to_string())
}

pub fn models_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(base_path(app)?.join("models"))
}

/// The tag the shipped binaries were built from. Read from the same manifest
/// the vendoring script uses, so the version the app reports and the version
/// on disk cannot drift apart.
const VENDOR_MANIFEST: &str = include_str!("../../scripts/vendor.json");

pub fn vendored_llama_tag() -> Option<String> {
    serde_json::from_str::<serde_json::Value>(VENDOR_MANIFEST)
        .ok()?
        .get("llamaCpp")?
        .get("tag")?
        .as_str()
        .map(str::to_string)
}

/// Picks between the two binaries that shipped in the installer. Both are
/// present on every install, so this makes no network call and the answer is a
/// property of the machine, not of the download.
///
/// Vulkan is probed first and CPU is the fallback for machines whose Vulkan
/// loader is missing entirely: the Vulkan build itself runs fine on CPU with
/// `-ngl 0`, so the fallback is the exception, not the rule (AD-022).
fn choose_backend(app: &AppHandle) -> Result<(Backend, PathBuf, i32), String> {
    let vulkan = bundled::llama_server(app, Backend::Vulkan)?;
    match probe_devices(&vulkan) {
        DeviceProbe::GpuAvailable(name) => {
            emit_stage(
                app,
                RuntimeStage::Preparing,
                Some(format!("GPU detectada: {name}")),
            );
            Ok((Backend::Vulkan, vulkan, -1))
        }
        DeviceProbe::CpuOnly => {
            emit_stage(
                app,
                RuntimeStage::Preparing,
                Some("Nenhuma GPU compatível encontrada — usando CPU".to_string()),
            );
            Ok((Backend::Vulkan, vulkan, 0))
        }
        DeviceProbe::BinaryFailed(reason) => {
            emit_stage(
                app,
                RuntimeStage::Preparing,
                Some(format!(
                    "Build Vulkan não executou ({reason}) — usando a versão CPU"
                )),
            );
            let cpu = bundled::llama_server(app, Backend::Cpu)?;
            // Both builds failing is a real dead end, and the reason from the
            // last probe is the only clue the user can act on.
            if let DeviceProbe::BinaryFailed(reason) = detect::probe_devices(&cpu) {
                return Err(format!("o llama-server não executa nesta máquina: {reason}"));
            }
            Ok((Backend::Cpu, cpu, 0))
        }
    }
}

/// Preparing keeps the model out of it on purpose. The model the user wants is
/// a separate, much larger decision, and tying the two together would make a
/// fresh install impossible without the network — which is exactly what this
/// milestone set out to fix (SELF-11).
#[tauri::command]
pub async fn prepare_runtime(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<RuntimeStatus, String> {
    if TargetOs::current().is_none() {
        return Err(RuntimeError::UnsupportedPlatform.to_string());
    }
    let models_dir = models_dir(&app)?;
    std::fs::create_dir_all(&models_dir).map_err(|e| e.to_string())?;

    emit_stage(&app, RuntimeStage::Preparing, None);
    let (backend, binary, probed_gpu_layers) = choose_backend(&app)?;

    // The model already chosen (or copied into the folder by hand) survives a
    // re-prepare: only the engine half of the row is rewritten.
    let row = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        let mut row = store::load(sql)?;
        row.release_tag = vendored_llama_tag();
        row.backend = Some(backend.as_str().to_string());
        row.binary_path = Some(binary.to_string_lossy().to_string());
        if row.gpu_layers.is_none() {
            row.gpu_layers = Some(probed_gpu_layers);
        }
        store::save(sql, &row)?;
        row
    };

    if !row.is_ready() {
        emit_stage(&app, RuntimeStage::NoModel, None);
        return Ok(status_from(&row, None, RuntimeStage::NoModel));
    }

    // EMBED-04 AC4: with engine and model in place the sidecar comes up on its
    // own, so the runtime reports "running" without another click. A failure
    // here still leaves a usable prepared state to start manually.
    match start_sidecar_from_row(&app, &row).await {
        Ok(port) => Ok(status_from(&row, Some(port), RuntimeStage::Running)),
        Err(e) => {
            let mut status = status_from(&row, None, RuntimeStage::Ready);
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
pub async fn start_runtime(
    app: AppHandle,
    db: State<'_, DbState>,
) -> Result<RuntimeStatus, String> {
    let row = {
        let guard = db.0.lock().map_err(|e| e.to_string())?;
        let sql = require_conn(&guard)?;
        store::load(sql)?
    };
    let port = start_sidecar_from_row(&app, &row).await?;
    Ok(status_from(&row, Some(port), RuntimeStage::Running))
}

#[tauri::command]
pub fn stop_runtime(app: AppHandle) -> Result<(), String> {
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
        store::set_config(sql, context_length, gpu_layers)?
    };

    if running_port(app).is_some() {
        stop_runtime(app.clone())?;
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
        store::set_active_model(sql, &wanted)?
    };

    if running_port(app).is_some() {
        if changed {
            stop_runtime(app.clone())?;
            start_sidecar_from_row(app, &row).await?;
        }
    } else if row.is_ready() {
        // Picking the first model is what turns a prepared runtime into a
        // usable one, so it starts here instead of leaving the user to hunt
        // for a Start button they were never told about.
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
    stage: RuntimeStage,
) -> RuntimeStatus {
    RuntimeStatus {
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
pub fn runtime_status(
    app: AppHandle,
    db: State<DbState>,
) -> Result<RuntimeStatus, String> {
    if TargetOs::current().is_none() {
        return Ok(RuntimeStatus {
            stage: RuntimeStage::Unsupported,
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

    // "Prepared but with no model" is its own state, not a broken install: it
    // names the one thing left to do instead of sending the user back to a
    // preparation step that already succeeded.
    let stage = if port.is_some() {
        RuntimeStage::Running
    } else if row.is_ready() {
        RuntimeStage::Ready
    } else if row.is_prepared() {
        RuntimeStage::NoModel
    } else {
        RuntimeStage::NotPrepared
    };

    Ok(status_from(&row, port, stage))
}

/// Progress of a GGUF download, keyed by the URL the caller asked for so the
/// card that started it is the one that shows the bar.
#[derive(Debug, Serialize, Clone)]
struct ModelDownloadProgress {
    identifier: String,
    progress: PullProgress,
}

fn forward_model_progress(
    app: &AppHandle,
    identifier: String,
) -> tokio::sync::mpsc::Sender<PullProgress> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<PullProgress>(32);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = app.emit(
                "model-download-progress",
                ModelDownloadProgress {
                    identifier: identifier.clone(),
                    progress,
                },
            );
        }
    });
    tx
}

/// EMBED-13: any direct `.gguf` link, downloaded into the same models folder
/// the sidecar already reads from. This is the only download the app makes on
/// the user's behalf now, and it is always a model they picked (SELF-11).
#[tauri::command]
pub async fn download_model(app: AppHandle, url: String) -> Result<(), String> {
    let models_dir = models_dir(&app)?;
    let progress = forward_model_progress(&app, url.clone());
    let result = model::download_model_from_url(&url, &models_dir, progress)
        .await
        .map(|_| ())
        .map_err(|e| e.to_string());

    // The success frame closes the progress bar; without it a finished
    // download would sit at whatever percentage the last chunk reported.
    let _ = app.emit(
        "model-download-progress",
        ModelDownloadProgress {
            identifier: url,
            progress: PullProgress {
                status: match &result {
                    Ok(()) => PullStatus::Success,
                    Err(_) => PullStatus::Error,
                },
                downloaded_bytes: None,
                total_bytes: None,
                message: result.as_ref().err().cloned(),
            },
        },
    );
    result
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
