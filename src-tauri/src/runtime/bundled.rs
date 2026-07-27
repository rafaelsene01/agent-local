// SPEC: self-contained-runtime (SELF-09, SELF-10, SELF-12, SELF-13)

use super::Backend;
use std::path::{Path, PathBuf};
use tauri::{Manager, path::BaseDirectory};

/// Everything under `src-tauri/resources/`, put there by `npm run vendor` and
/// copied into the bundle by Tauri. Resolved through the path resolver rather
/// than guessed from the executable's location, because the answer differs per
/// platform: the exe directory on Windows, `/usr/lib/<exe>` on an installed
/// Linux package, `${APPDIR}/usr/lib/<exe>` inside an AppImage.
pub fn resource_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resolve("resources", BaseDirectory::Resource)
        .map_err(|e| format!("não foi possível localizar os recursos do app: {e}"))
}

/// Recursive lookup by file name. The vendors decide their own layout — pdfium
/// nests under `bin/`, the ONNX Runtime under `<version>/lib/`, llama.cpp
/// flat — and that layout is theirs to change between releases.
pub fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut dirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        } else if path.file_name().is_some_and(|n| n == name) {
            return Some(path);
        }
    }
    dirs.iter().find_map(|d| find_file(d, name))
}

/// A missing component is a broken installation, never a reason to reach for
/// the network: naming the file and the folder is what lets the user (or a bug
/// report) tell "the installer is incomplete" from "the app is confused".
fn missing(what: &str, dir: &Path) -> String {
    format!(
        "componente '{what}' não encontrado em {} — reinstale o LocalMind",
        dir.display()
    )
}

pub fn llama_server(app: &tauri::AppHandle, backend: Backend) -> Result<PathBuf, String> {
    let name = if cfg!(windows) {
        "llama-server.exe"
    } else {
        "llama-server"
    };
    let dir = resource_root(app)?.join("llama").join(backend.as_str());
    let found = find_file(&dir, name).ok_or_else(|| missing(name, &dir))?;
    ensure_executable(&found, &fallback_dir(app, backend))
}

pub fn onnxruntime_dylib(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let name = if cfg!(windows) {
        "onnxruntime.dll"
    } else {
        "libonnxruntime.so"
    };
    let dir = resource_root(app)?.join("onnxruntime");
    find_file(&dir, name).ok_or_else(|| missing(name, &dir))
}

pub fn pdfium_library(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let name = if cfg!(windows) {
        "pdfium.dll"
    } else {
        "libpdfium.so"
    };
    let dir = resource_root(app)?.join("pdfium");
    find_file(&dir, name).ok_or_else(|| missing(name, &dir))
}

/// Where a backend is copied when the packaged one cannot be made executable.
fn fallback_dir(app: &tauri::AppHandle, backend: Backend) -> PathBuf {
    crate::config::load_config(app)
        .ok()
        .flatten()
        .map(|cfg| cfg.base_path_buf())
        .unwrap_or_else(std::env::temp_dir)
        .join("runtime")
        .join("llama")
        .join(backend.as_str())
}

#[cfg(windows)]
pub fn ensure_executable(path: &Path, _fallback_dir: &Path) -> Result<PathBuf, String> {
    // Windows has no execute bit; the file either exists or it doesn't.
    Ok(path.to_path_buf())
}

/// Whether `bundle.resources` preserves the execute bit inside a `.deb` or an
/// AppImage is not documented anywhere we could find, so this does not depend
/// on the answer: it sets the bit, and if the packaged location is read-only
/// (`/usr/lib/LocalMind` belongs to root) it copies the backend folder into the
/// user's base folder and marks it there.
///
/// The whole folder is copied, not just the executable: `llama-server` loads
/// the `libggml-*.so` files sitting next to it.
#[cfg(unix)]
pub fn ensure_executable(path: &Path, fallback_dir: &Path) -> Result<PathBuf, String> {
    use std::os::unix::fs::PermissionsExt;

    let mode = std::fs::metadata(path)
        .map_err(|e| format!("não foi possível ler {}: {e}", path.display()))?
        .permissions()
        .mode();
    if mode & 0o111 != 0 {
        return Ok(path.to_path_buf());
    }

    if std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).is_ok() {
        return Ok(path.to_path_buf());
    }

    let source_dir = path
        .parent()
        .ok_or_else(|| format!("{} não tem diretório pai", path.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("{} não tem nome de arquivo", path.display()))?;

    copy_tree_executable(source_dir, fallback_dir)?;
    let copied = fallback_dir.join(file_name);
    std::fs::set_permissions(&copied, std::fs::Permissions::from_mode(0o755))
        .map_err(|e| format!("não foi possível marcar {} como executável: {e}", copied.display()))?;
    eprintln!(
        "runtime: {} não era executável e a pasta era só-leitura; usando a cópia em {}",
        path.display(),
        copied.display()
    );
    Ok(copied)
}

#[cfg(unix)]
fn copy_tree_executable(from: &Path, to: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(from).map_err(|e| e.to_string())?.flatten() {
        let source = entry.path();
        let destination = to.join(entry.file_name());
        if source.is_dir() {
            copy_tree_executable(&source, &destination)?;
        } else {
            std::fs::copy(&source, &destination).map_err(|e| {
                format!("não foi possível copiar {}: {e}", source.display())
            })?;
            let _ = std::fs::set_permissions(&destination, std::fs::Permissions::from_mode(0o755));
        }
    }
    Ok(())
}

/// The four folders earlier versions downloaded into (~150 MB) and that
/// nothing reads any more (SELF-18).
///
/// Named one by one instead of wiping `<base>/runtime/` wholesale: the sidecar
/// log lives right there, and it is the only diagnostic left now that the
/// console window is hidden — deleting it on every boot would undo M7.1.
const LEGACY_DOWNLOAD_DIRS: [&str; 4] = ["vulkan", "cpu", "onnxruntime", "pdfium"];

/// Returns how many were removed. Failure is ignored on purpose: a locked file
/// is not a reason to keep the user out of the app.
pub fn remove_legacy_downloads(runtime_dir: &Path) -> usize {
    LEGACY_DOWNLOAD_DIRS
        .iter()
        .filter(|name| {
            let path = runtime_dir.join(name);
            path.is_dir() && std::fs::remove_dir_all(&path).is_ok()
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("localmind-bundled-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn finds_a_file_nested_under_a_vendor_specific_layout() {
        let root = temp_dir("find");
        let nested = root.join("onnxruntime-win-x64-1.28.0").join("lib");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("onnxruntime.dll"), b"stub").unwrap();

        let found = find_file(&root, "onnxruntime.dll").expect("the nested library is found");
        assert_eq!(found, nested.join("onnxruntime.dll"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_file_is_none_rather_than_a_guessed_path() {
        let root = temp_dir("missing");
        std::fs::write(root.join("readme.txt"), b"nothing here").unwrap();
        assert_eq!(find_file(&root, "pdfium.dll"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The message has to name both the file and where it was looked for, and
    /// must never suggest downloading it — that path no longer exists.
    #[test]
    fn the_missing_component_message_names_the_file_and_the_folder() {
        let message = missing("llama-server.exe", Path::new("/opt/LocalMind/resources/llama/vulkan"));
        assert!(message.contains("llama-server.exe"));
        assert!(message.contains("/opt/LocalMind/resources/llama/vulkan"));
        assert!(message.contains("reinstale"));
        assert!(
            !message.to_lowercase().contains("baix"),
            "a missing component must not be reported as something to download"
        );
    }

    #[test]
    fn the_cleanup_removes_the_four_download_folders_and_nothing_else() {
        let base = temp_dir("legacy");
        let runtime = base.join("runtime");
        for name in ["vulkan", "cpu", "onnxruntime", "pdfium"] {
            std::fs::create_dir_all(runtime.join(name)).unwrap();
            std::fs::write(runtime.join(name).join("blob.bin"), b"old").unwrap();
        }
        // Things that must survive: the sidecar log (M7.1) and the fallback
        // copy of a bundled backend.
        std::fs::write(runtime.join("llama-server.log"), b"boot").unwrap();
        std::fs::create_dir_all(runtime.join("llama").join("vulkan")).unwrap();
        std::fs::create_dir_all(base.join("models")).unwrap();
        std::fs::write(base.join("models").join("phi.gguf"), b"model").unwrap();

        assert_eq!(remove_legacy_downloads(&runtime), 4);

        for name in ["vulkan", "cpu", "onnxruntime", "pdfium"] {
            assert!(!runtime.join(name).exists(), "{name} should be gone");
        }
        assert!(runtime.join("llama-server.log").exists(), "the log is the only diagnostic left");
        assert!(runtime.join("llama").join("vulkan").exists());
        assert!(base.join("models").join("phi.gguf").exists(), "models are never touched");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn cleaning_a_folder_that_was_never_downloaded_into_is_silent() {
        let base = temp_dir("legacy-empty");
        assert_eq!(remove_legacy_downloads(&base.join("runtime")), 0);
        assert_eq!(remove_legacy_downloads(&base), 0);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[cfg(windows)]
    #[test]
    fn on_windows_ensure_executable_is_a_no_op() {
        let root = temp_dir("noop");
        let file = root.join("llama-server.exe");
        std::fs::write(&file, b"stub").unwrap();
        assert_eq!(ensure_executable(&file, &root.join("fallback")).unwrap(), file);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn an_already_executable_file_is_returned_untouched() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_dir("exec");
        let file = root.join("llama-server");
        std::fs::write(&file, b"stub").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755)).unwrap();

        assert_eq!(ensure_executable(&file, &root.join("fallback")).unwrap(), file);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn a_file_without_the_bit_gets_it_in_place_when_the_folder_is_writable() {
        use std::os::unix::fs::PermissionsExt;
        let root = temp_dir("chmod");
        let file = root.join("llama-server");
        std::fs::write(&file, b"stub").unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

        let resolved = ensure_executable(&file, &root.join("fallback")).unwrap();
        assert_eq!(resolved, file, "no copy is needed when chmod works");
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert!(mode & 0o111 != 0);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The whole backend folder travels, not just the executable: the server
    /// loads the `libggml-*.so` files that sit beside it.
    #[cfg(unix)]
    #[test]
    fn the_copy_fallback_carries_the_sibling_libraries() {
        let root = temp_dir("copy");
        let source = root.join("vulkan");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("llama-server"), b"stub").unwrap();
        std::fs::write(source.join("libggml-vulkan.so"), b"lib").unwrap();

        let destination = root.join("fallback");
        copy_tree_executable(&source, &destination).unwrap();

        assert!(destination.join("llama-server").exists());
        assert!(
            destination.join("libggml-vulkan.so").exists(),
            "a server without its shared libraries cannot start"
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
