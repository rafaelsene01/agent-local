//! In-place update for the portable bundle.
//!
//! Windows does not let a running `.exe` be overwritten — but it *does* let it
//! be renamed. That single fact is why this needs no helper process: rename the
//! running binary out of the way, move the new one in, relaunch, and delete the
//! leftover on the next boot. Nothing is written outside the app folder and
//! nothing touches the registry, which is what makes "no administrator" a
//! property of the design rather than a hope.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Emitter};

use super::manifest::PlatformEntry;
use super::signature;
use crate::providers::PullProgress;

pub const UPDATE_PROGRESS_EVENT: &str = "update-download-progress";

/// Where the archive is unpacked before the swap. Hidden-ish name so it does
/// not look like part of the app if the user peeks mid-update.
const STAGING_DIR: &str = ".update";
const OLD_SUFFIX: &str = ".old";

#[derive(Clone, serde::Serialize)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

/// Refuses before downloading hundreds of megabytes we could never install:
/// a portable copy on a write-protected stick, or dropped into `Program Files`.
pub fn ensure_writable(dir: &Path) -> Result<(), String> {
    let probe = dir.join(".localmind-update-test");
    fs::write(&probe, b"ok").map_err(|_| {
        format!(
            "não é possível atualizar a partir de '{}': a pasta não permite escrita. \
             Mova o LocalMind para uma pasta sua (Documentos, Desktop) e tente de novo.",
            dir.display()
        )
    })?;
    let _ = fs::remove_file(&probe);
    Ok(())
}

/// The archive wraps everything in a single `LocalMind/` folder so unzipping it
/// by hand does not scatter files; installing it means dropping that wrapper.
pub fn strip_first_component(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    let rest = normalized.split_once('/')?.1;
    let rest = rest.trim_start_matches('/');
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

/// Zip entries are attacker-controlled input in principle; a `..` component
/// would let an archive write outside the app folder.
pub fn is_safe_relative(path: &str) -> bool {
    !path.is_empty()
        && !Path::new(path).is_absolute()
        && !path.split('/').any(|part| part == ".." || part.contains(':'))
}

/// Removes the leftovers of a previous update. Called at boot; every failure is
/// ignored on purpose — a stale file must never keep the app from starting.
pub fn cleanup_old_files(app_dir: &Path) {
    let _ = fs::remove_dir_all(app_dir.join(STAGING_DIR));

    let Ok(entries) = fs::read_dir(app_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some(&OLD_SUFFIX[1..]) {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Downloads, verifies and installs the portable bundle.
///
/// Returns the path of the executable to relaunch. The caller spawns it and
/// exits — `AppHandle::restart` cannot be used, because it relaunches
/// `current_exe()`, which by then points at the renamed `.old` file.
pub async fn install(
    app: &AppHandle,
    app_dir: &Path,
    entry: &PlatformEntry,
    pubkey: &str,
) -> Result<PathBuf, String> {
    ensure_writable(app_dir)?;

    // Captured before anything moves. Windows caches the image path, so
    // `current_exe()` after the rename is not something to rely on either way —
    // reading it once removes the question.
    let exe_name = current_exe_name()
        .ok_or_else(|| "não foi possível localizar o executável atual".to_string())?;

    let temp = std::env::temp_dir().join(format!("localmind-update-{}", std::process::id()));
    let _ = fs::remove_dir_all(&temp);
    fs::create_dir_all(&temp).map_err(|e| format!("erro de arquivo: {e}"))?;

    let archive = temp.join("portable.zip");
    let downloaded = download(app, &entry.url, &archive).await;
    if let Err(e) = downloaded {
        let _ = fs::remove_dir_all(&temp);
        return Err(e);
    }

    // Verify BEFORE touching anything installed. A bad signature must leave the
    // current version completely untouched.
    let bytes = fs::read(&archive).map_err(|e| format!("erro de arquivo: {e}"))?;
    if let Err(e) = signature::verify(&bytes, &entry.signature, pubkey) {
        let _ = fs::remove_dir_all(&temp);
        return Err(format!("atualização recusada: {e}"));
    }
    drop(bytes);

    let staging = app_dir.join(STAGING_DIR);
    let _ = fs::remove_dir_all(&staging);
    if let Err(e) = extract_stripping_root(&archive, &staging) {
        let _ = fs::remove_dir_all(&temp);
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }
    let _ = fs::remove_dir_all(&temp);

    let result = swap(app_dir, &staging, &exe_name);
    let _ = fs::remove_dir_all(&staging);
    result?;

    Ok(app_dir.join(&exe_name))
}

async fn download(app: &AppHandle, url: &str, dest: &Path) -> Result<(), String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<PullProgress>(16);

    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = handle.emit(
                UPDATE_PROGRESS_EVENT,
                DownloadProgress {
                    downloaded: progress.downloaded_bytes.unwrap_or(0),
                    total: progress.total_bytes,
                },
            );
        }
    });

    // Reuses the runtime downloader: it already writes to `<dest>.part` and only
    // renames on success, so an interrupted download leaves nothing usable.
    crate::runtime::download::download_with_progress(url, dest, tx)
        .await
        .map_err(|e| e.to_string())
}

fn extract_stripping_root(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| format!("erro de arquivo: {e}"))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| format!("pacote inválido: {e}"))?;

    for index in 0..zip.len() {
        let mut item = zip
            .by_index(index)
            .map_err(|e| format!("pacote inválido: {e}"))?;

        let Some(relative) = strip_first_component(item.name()) else {
            continue; // the wrapper directory entry itself
        };
        if !is_safe_relative(&relative) {
            return Err(format!("pacote contém um caminho inseguro: {relative}"));
        }

        let target = dest.join(&relative);
        if item.is_dir() {
            fs::create_dir_all(&target).map_err(|e| format!("erro de arquivo: {e}"))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("erro de arquivo: {e}"))?;
        }
        let mut out = File::create(&target).map_err(|e| format!("erro de arquivo: {e}"))?;
        io::copy(&mut item, &mut out).map_err(|e| format!("erro de arquivo: {e}"))?;
    }

    Ok(())
}

fn current_exe_name() -> Option<std::ffi::OsString> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.file_name().map(|name| name.to_os_string()))
}

/// Renames the running executable aside, then moves the new files in. On any
/// failure the old executable is put back, so the app stays usable on the old
/// version instead of ending up half-updated.
fn swap(app_dir: &Path, staging: &Path, exe_name: &std::ffi::OsStr) -> Result<(), String> {
    let live = app_dir.join(exe_name);

    let mut retired = live.clone().into_os_string();
    retired.push(OLD_SUFFIX);
    let retired = PathBuf::from(retired);
    let _ = fs::remove_file(&retired);

    // Allowed while the process is running; overwriting it would not be.
    fs::rename(&live, &retired)
        .map_err(|e| format!("não foi possível liberar o executável atual: {e}"))?;

    match move_tree(staging, app_dir) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&live);
            let _ = fs::rename(&retired, &live);
            Err(format!(
                "falha ao aplicar a atualização, versão anterior restaurada: {e}"
            ))
        }
    }
}

fn move_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let target = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            move_tree(&entry.path(), &target)?;
        } else {
            let _ = fs::remove_file(&target);
            if fs::rename(entry.path(), &target).is_err() {
                // Different volume: rename fails, copy is the fallback.
                fs::copy(entry.path(), &target)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("localmind-update-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn strips_the_wrapper_directory() {
        assert_eq!(
            strip_first_component("LocalMind/LocalMind.exe").as_deref(),
            Some("LocalMind.exe")
        );
        assert_eq!(
            strip_first_component("LocalMind/sub/dir/file.txt").as_deref(),
            Some("sub/dir/file.txt")
        );
        // Zip files written on Windows can carry backslashes.
        assert_eq!(
            strip_first_component("LocalMind\\.portable").as_deref(),
            Some(".portable")
        );
    }

    #[test]
    fn the_wrapper_entry_itself_has_nothing_left() {
        assert_eq!(strip_first_component("LocalMind/"), None);
        assert_eq!(strip_first_component("LocalMind"), None);
        assert_eq!(strip_first_component(""), None);
    }

    #[test]
    fn traversal_paths_are_rejected() {
        assert!(!is_safe_relative("../evil.exe"));
        assert!(!is_safe_relative("sub/../../evil.exe"));
        assert!(!is_safe_relative("C:/Windows/evil.exe"));
        assert!(!is_safe_relative(""));
        assert!(is_safe_relative("LocalMind.exe"));
        assert!(is_safe_relative("sub/dir/file.txt"));
    }

    #[test]
    fn cleanup_removes_leftovers_and_leaves_the_app_alone() {
        let dir = temp_dir("cleanup");
        fs::write(dir.join("LocalMind.exe"), b"new").unwrap();
        fs::write(dir.join("LocalMind.exe.old"), b"old").unwrap();
        fs::write(dir.join("README.txt"), b"keep").unwrap();
        fs::create_dir_all(dir.join(STAGING_DIR)).unwrap();
        fs::write(dir.join(STAGING_DIR).join("leftover"), b"x").unwrap();

        cleanup_old_files(&dir);

        assert!(dir.join("LocalMind.exe").exists());
        assert!(dir.join("README.txt").exists());
        assert!(!dir.join("LocalMind.exe.old").exists());
        assert!(!dir.join(STAGING_DIR).exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_on_a_missing_folder_is_silent() {
        cleanup_old_files(Path::new("/definitely/not/a/real/path"));
    }

    #[test]
    fn move_tree_overwrites_and_recurses() {
        let root = temp_dir("movetree");
        let src = root.join("src");
        let dst = root.join("dst");
        fs::create_dir_all(src.join("nested")).unwrap();
        fs::create_dir_all(&dst).unwrap();
        fs::write(src.join("a.txt"), b"new").unwrap();
        fs::write(src.join("nested").join("b.txt"), b"nested").unwrap();
        fs::write(dst.join("a.txt"), b"old").unwrap();

        move_tree(&src, &dst).unwrap();

        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "new");
        assert_eq!(
            fs::read_to_string(dst.join("nested").join("b.txt")).unwrap(),
            "nested"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_writable_folder_passes_and_the_probe_is_cleaned_up() {
        let dir = temp_dir("writable");
        ensure_writable(&dir).unwrap();
        assert!(!dir.join(".localmind-update-test").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_folder_that_does_not_exist_is_refused_before_downloading() {
        let err = ensure_writable(Path::new("/definitely/not/a/real/path")).unwrap_err();
        assert!(err.contains("não permite escrita"), "unexpected: {err}");
    }

    #[test]
    fn extraction_drops_the_wrapper_folder() {
        use std::io::Write;

        let dir = temp_dir("extract");
        let archive = dir.join("portable.zip");
        {
            let file = File::create(&archive).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            let opts = zip::write::SimpleFileOptions::default();
            writer.start_file::<_, ()>("LocalMind/LocalMind.exe", opts).unwrap();
            writer.write_all(b"binary").unwrap();
            writer.start_file::<_, ()>("LocalMind/.portable", opts).unwrap();
            writer.write_all(b"").unwrap();
            writer.finish().unwrap();
        }

        let out = dir.join("staged");
        extract_stripping_root(&archive, &out).unwrap();

        assert_eq!(fs::read_to_string(out.join("LocalMind.exe")).unwrap(), "binary");
        assert!(out.join(".portable").exists());
        assert!(!out.join("LocalMind").exists(), "wrapper must be dropped");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn extraction_refuses_a_traversal_entry() {
        use std::io::Write;

        let dir = temp_dir("traversal");
        let archive = dir.join("evil.zip");
        {
            let file = File::create(&archive).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            writer
                .start_file::<_, ()>("LocalMind/../evil.exe", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"pwned").unwrap();
            writer.finish().unwrap();
        }

        let err = extract_stripping_root(&archive, &dir.join("staged")).unwrap_err();
        assert!(err.contains("inseguro"), "unexpected: {err}");

        let _ = fs::remove_dir_all(&dir);
    }
}
