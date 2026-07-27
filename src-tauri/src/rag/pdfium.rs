// SPEC: self-contained-runtime (SELF-12)

use crate::runtime::bundled;
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::AppHandle;

/// pdfium replaced `pdf-extract` 0.12, which silently dropped whole glyphs from
/// this project's own corpus: `q`, `v`, `x`, `b`, `f` and every accented vowel
/// vanished from 51% of the chunks of a Código Civil PDF ("salvo se o exercício
/// da profissão" came out as "salo se o eerccio da profisso"). pdfium reads the
/// same file with zero losses, measured against poppler as a reference.
///
/// It used to be downloaded on first use; it now ships in the installer, and
/// the pinned release lives in `scripts/vendor.json`.

/// Set once the library is resolved; `extract_text` is synchronous and has no
/// `AppHandle`, so the path has to outlive the lookup. Same shape as
/// `embedding::MODEL_CACHE_DIR`.
static LIBRARY_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// Only PDFs need the library, so importing a `.txt` never touches it.
pub async fn ensure_for(app: &AppHandle, path: &Path) -> Result<(), String> {
    if super::parsing::extension_of(path) != "pdf" {
        return Ok(());
    }
    ensure_library(app).await.map(|_| ())
}

/// Still `async` because the document pipeline calls it that way; there is no
/// I/O left to await beyond resolving a path.
pub async fn ensure_library(app: &AppHandle) -> Result<PathBuf, String> {
    let library = bundled::pdfium_library(app)?;
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
    fn a_non_pdf_needs_no_library() {
        // `ensure_for` short-circuits on extension before resolving anything,
        // which is what makes importing a .txt work on a broken install too.
        assert_eq!(super::super::parsing::extension_of(Path::new("nota.txt")), "txt");
        assert_eq!(super::super::parsing::extension_of(Path::new("a.PDF")), "pdf");
    }

    /// Reading a PDF with no library loaded has to say so, not panic or return
    /// empty text that would be indexed as a valid (blank) document.
    #[test]
    fn extracting_before_the_library_is_resolved_fails_with_a_message() {
        if LIBRARY_PATH.lock().unwrap().is_some() {
            // Another test in this process already resolved it; the assertion
            // below would then be testing nothing.
            return;
        }
        let error = extract_text(Path::new("qualquer.pdf")).unwrap_err();
        assert!(error.contains("leitor de PDF"), "unexpected message: {error}");
    }
}
