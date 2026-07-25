use crate::runtime::download;
use std::path::{Path, PathBuf};
use tauri::AppHandle;

/// Pinned release, confirmed live on 2026-07-25 (asset present, ~79 MB).
/// ONNX Runtime's C API is backward compatible, so a newer runtime still
/// serves the older API version `ort` asks for.
const ORT_VERSION: &str = "1.28.0";

fn asset_url() -> Option<String> {
    let file = if cfg!(windows) {
        format!("onnxruntime-win-x64-{ORT_VERSION}.zip")
    } else if cfg!(target_os = "linux") {
        format!("onnxruntime-linux-x64-{ORT_VERSION}.tgz")
    } else {
        return None;
    };
    Some(format!(
        "https://github.com/microsoft/onnxruntime/releases/download/v{ORT_VERSION}/{file}"
    ))
}

fn dylib_name() -> &'static str {
    if cfg!(windows) {
        "onnxruntime.dll"
    } else {
        "libonnxruntime.so"
    }
}

fn find_dylib(dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else if path.file_name().is_some_and(|n| n == dylib_name()) {
            return Some(path);
        }
    }
    dirs.iter().find_map(|d| find_dylib(d))
}

/// fastembed is built against a dynamically loaded ONNX Runtime, so the
/// library has to exist before the first embedding call and `ORT_DYLIB_PATH`
/// has to point at it. Static linking was ruled out because the prebuilt
/// static lib requires the MSVC 2022 STL.
///
/// Downloading rather than bundling keeps the installer small — the same
/// trade-off already made for the llama.cpp sidecar (AD-022).
pub async fn ensure_dylib(app: &AppHandle) -> Result<PathBuf, String> {
    let cfg = crate::config::load_config(app)?
        .ok_or_else(|| "Nenhuma pasta de armazenamento configurada ainda".to_string())?;
    let dir = cfg.base_path_buf().join("runtime").join("onnxruntime");

    if let Some(existing) = find_dylib(&dir) {
        set_dylib_path(&existing);
        return Ok(existing);
    }

    let url = asset_url().ok_or_else(|| {
        "o motor de embeddings só está disponível para Windows e Linux".to_string()
    })?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let archive_name = url.rsplit('/').next().unwrap_or("onnxruntime-archive");
    let archive = dir.join(archive_name);
    // No progress consumer here: this runs once, inside the document pipeline,
    // which already reports its own "embedding" stage to the UI.
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    tauri::async_runtime::spawn(async move { while rx.recv().await.is_some() {} });

    download::download_with_progress(&url, &archive, tx)
        .await
        .map_err(|e| format!("falha ao baixar o ONNX Runtime: {e}"))?;
    download::extract(&archive, &dir).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&archive);

    let dylib = find_dylib(&dir).ok_or_else(|| {
        format!(
            "o pacote do ONNX Runtime não contém {} em {}",
            dylib_name(),
            dir.display()
        )
    })?;
    set_dylib_path(&dylib);
    Ok(dylib)
}

fn set_dylib_path(path: &Path) {
    // `ort` reads this the first time a session is created, so setting it
    // before any embedding call is enough.
    std::env::set_var("ORT_DYLIB_PATH", path);
}
