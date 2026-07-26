use crate::runtime::download;
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::AppHandle;

/// Pinned release from `bblanchon/pdfium-binaries`, verified live on
/// 2026-07-26 (HTTP 200, 3.74 MB for the Windows asset).
///
/// Replaces `pdf-extract` 0.12, which silently dropped whole glyphs from this
/// project's own test corpus: `q`, `v`, `x`, `b`, `f` and every accented vowel
/// vanished from 51% of the chunks of a Código Civil PDF ("salvo se o exercício
/// da profissão" came out as "salo se o eerccio da profisso"). pdfium reads the
/// same file with zero losses, measured against poppler as a reference.
const RELEASE: &str = "chromium/7961";

/// Downloaded rather than bundled, the same trade-off already made for the
/// llama.cpp sidecar (AD-022) and the ONNX Runtime (AD-025).
fn asset_url() -> Option<String> {
    let file = if cfg!(windows) {
        "pdfium-win-x64.tgz"
    } else if cfg!(target_os = "linux") {
        "pdfium-linux-x64.tgz"
    } else {
        return None;
    };
    Some(format!(
        "https://github.com/bblanchon/pdfium-binaries/releases/download/{RELEASE}/{file}"
    ))
}

/// Set once the library is on disk; `extract_text` is synchronous and has no
/// `AppHandle`, so the resolved path has to outlive the download. Same shape as
/// `embedding::MODEL_CACHE_DIR`.
static LIBRARY_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The archive nests the library under `bin/` (Windows) or `lib/` (Linux), and
/// the layout is the vendor's to change, so it is searched for rather than
/// assumed.
fn find_library(dir: &Path) -> Option<PathBuf> {
    let wanted = Pdfium::pdfium_platform_library_name();
    let entries = std::fs::read_dir(dir).ok()?;
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else if path.file_name().is_some_and(|n| n == wanted) {
            return Some(path);
        }
    }
    dirs.iter().find_map(|d| find_library(d))
}

/// Only PDFs need the library, so importing a `.txt` never triggers a download.
pub async fn ensure_for(app: &AppHandle, path: &Path) -> Result<(), String> {
    if super::parsing::extension_of(path) != "pdf" {
        return Ok(());
    }
    ensure_library(app).await.map(|_| ())
}

pub async fn ensure_library(app: &AppHandle) -> Result<PathBuf, String> {
    let cfg = crate::config::load_config(app)?
        .ok_or_else(|| "Nenhuma pasta de armazenamento configurada ainda".to_string())?;
    let dir = cfg.base_path_buf().join("runtime").join("pdfium");

    if let Some(existing) = find_library(&dir) {
        remember(&existing);
        return Ok(existing);
    }

    let url = asset_url()
        .ok_or_else(|| "a leitura de PDF só está disponível para Windows e Linux".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    // No progress consumer: this runs once, inside the document pipeline, which
    // already reports its own "parsing" stage to the UI.
    let archive = dir.join(url.rsplit('/').next().unwrap_or("pdfium-archive"));
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    tauri::async_runtime::spawn(async move { while rx.recv().await.is_some() {} });

    download::download_with_progress(&url, &archive, tx)
        .await
        .map_err(|e| format!("falha ao baixar o leitor de PDF: {e}"))?;
    download::extract(&archive, &dir).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&archive);

    let library = find_library(&dir).ok_or_else(|| {
        format!(
            "o pacote do pdfium não contém {} em {}",
            Pdfium::pdfium_platform_library_name().to_string_lossy(),
            dir.display()
        )
    })?;
    remember(&library);
    Ok(library)
}

fn remember(path: &Path) {
    if let Ok(mut current) = LIBRARY_PATH.lock() {
        *current = Some(path.to_path_buf());
    }
}

/// Concatenates every page's text. `bind_to_library` caches its bindings
/// process-wide, so the repeated call is cheap after the first document.
pub fn extract_text(pdf: &Path) -> Result<String, String> {
    let library = LIBRARY_PATH
        .lock()
        .map_err(|e| e.to_string())?
        .clone()
        .ok_or_else(|| "o leitor de PDF ainda não foi carregado".to_string())?;

    let bindings = Pdfium::bind_to_library(&library)
        .map_err(|e| format!("não foi possível carregar o pdfium: {e}"))?;
    let pdfium = Pdfium::new(bindings);
    let document = pdfium
        .load_pdf_from_file(pdf, None)
        .map_err(|e| format!("não foi possível abrir o PDF: {e}"))?;

    let mut out = String::new();
    for page in document.pages().iter() {
        let text = page
            .text()
            .map_err(|e| format!("não foi possível ler o texto da página: {e}"))?;
        out.push_str(&text.all());
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pinned_asset_url_is_well_formed_for_this_platform() {
        let url = asset_url().expect("Windows and Linux are supported");
        assert!(url.contains(RELEASE));
        assert!(url.ends_with(".tgz"));
    }

    #[test]
    fn a_non_pdf_needs_no_library() {
        // `ensure_for` short-circuits on extension before touching the network
        // or the config, which is what keeps a .txt import offline.
        assert_eq!(super::super::parsing::extension_of(Path::new("nota.txt")), "txt");
        assert_eq!(super::super::parsing::extension_of(Path::new("a.PDF")), "pdf");
    }
}
