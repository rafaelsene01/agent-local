use super::RuntimeError;
use crate::providers::{PullProgress, PullStatus};
use futures_util::StreamExt;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::Sender;

/// Headroom on top of the reported size, for the extracted archive and
/// filesystem overhead — the download is refused before it starts if the
/// disk can't plausibly hold it.
const FREE_SPACE_MARGIN: f64 = 1.5;

fn part_path(dest: &Path) -> PathBuf {
    let mut name = dest.as_os_str().to_os_string();
    name.push(".part");
    PathBuf::from(name)
}

/// Downloads to `<dest>.part` and only renames on success, so a dropped
/// connection never leaves a truncated binary or model looking valid.
pub async fn download_with_progress(
    url: &str,
    dest: &Path,
    progress: Sender<PullProgress>,
) -> Result<(), RuntimeError> {
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| RuntimeError::Network(e.to_string()))?;

    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| RuntimeError::Network(e.to_string()))?;

    if !resp.status().is_success() {
        return Err(RuntimeError::Network(format!(
            "o servidor respondeu {} ao baixar {url}",
            resp.status()
        )));
    }

    let total_bytes = resp.content_length();
    if let (Some(total), Some(parent)) = (total_bytes, dest.parent()) {
        ensure_free_space(parent, total)?;
    }

    let part = part_path(dest);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| RuntimeError::Io(e.to_string()))?;
    }
    let mut file = File::create(&part).map_err(|e| RuntimeError::Io(e.to_string()))?;

    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                let _ = fs::remove_file(&part);
                return Err(RuntimeError::Network(e.to_string()));
            }
        };
        if let Err(e) = io::Write::write_all(&mut file, &chunk) {
            let _ = fs::remove_file(&part);
            return Err(RuntimeError::Io(e.to_string()));
        }
        downloaded += chunk.len() as u64;
        let _ = progress
            .send(PullProgress {
                status: PullStatus::Downloading,
                downloaded_bytes: Some(downloaded),
                total_bytes,
                message: None,
            })
            .await;
    }

    drop(file);
    fs::rename(&part, dest).map_err(|e| RuntimeError::Io(e.to_string()))?;

    let _ = progress
        .send(PullProgress {
            status: PullStatus::Success,
            downloaded_bytes: Some(downloaded),
            total_bytes,
            message: None,
        })
        .await;
    Ok(())
}

/// Best effort: filesystems that don't report free space simply don't block
/// the download, since failing on an unknown is worse than trying.
fn ensure_free_space(dir: &Path, needed: u64) -> Result<(), RuntimeError> {
    let required = (needed as f64 * FREE_SPACE_MARGIN) as u64;
    let Some(available) = available_space(dir) else {
        return Ok(());
    };
    if available < required {
        return Err(RuntimeError::Io(format!(
            "espaço em disco insuficiente: precisa de ~{:.1} GB livres, há {:.1} GB",
            required as f64 / 1e9,
            available as f64 / 1e9
        )));
    }
    Ok(())
}

fn available_space(dir: &Path) -> Option<u64> {
    use sysinfo::Disks;
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|d| dir.starts_with(d.mount_point()))
        // Longest mount point wins: on Linux every path also starts with "/".
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map(|d| d.available_space())
}

pub fn extract(archive: &Path, dest: &Path) -> Result<(), RuntimeError> {
    let name = archive.to_string_lossy().to_lowercase();
    if name.ends_with(".zip") {
        extract_zip(archive, dest)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        extract_tar_gz(archive, dest)
    } else {
        Err(RuntimeError::Io(format!(
            "formato de arquivo não suportado: {}",
            archive.display()
        )))
    }
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), RuntimeError> {
    let file = File::open(archive).map_err(|e| RuntimeError::Io(e.to_string()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| RuntimeError::Io(e.to_string()))?;
    zip.extract(dest).map_err(|e| RuntimeError::Io(e.to_string()))
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<(), RuntimeError> {
    let file = File::open(archive).map_err(|e| RuntimeError::Io(e.to_string()))?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);
    tar.unpack(dest).map_err(|e| RuntimeError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("localmind-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn extracts_a_zip_archive() {
        let dir = temp_dir("zip");
        let archive = dir.join("bundle.zip");
        {
            let file = File::create(&archive).unwrap();
            let mut writer = zip::ZipWriter::new(file);
            writer
                .start_file::<_, ()>("llama-server.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"binary").unwrap();
            writer.finish().unwrap();
        }

        let out = dir.join("out");
        extract(&archive, &out).unwrap();

        assert_eq!(
            fs::read_to_string(out.join("llama-server.txt")).unwrap(),
            "binary"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_an_unknown_archive_format() {
        let dir = temp_dir("unknown");
        let archive = dir.join("bundle.7z");
        fs::write(&archive, b"x").unwrap();

        let err = extract(&archive, &dir.join("out")).unwrap_err();

        assert!(matches!(err, RuntimeError::Io(_)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn an_aborted_download_leaves_no_final_file() {
        let dir = temp_dir("aborted");
        let dest = dir.join("model.gguf");
        let (tx, _rx) = tokio::sync::mpsc::channel(8);

        // Unroutable host: the request fails before any byte is written.
        let result = download_with_progress("http://127.0.0.1:1/model.gguf", &dest, tx).await;

        assert!(result.is_err());
        assert!(!dest.exists(), "the final path must never appear on failure");
        assert!(!part_path(&dest).exists(), "the .part file must be cleaned up");
        let _ = fs::remove_dir_all(&dir);
    }
}
