//! Where the sidecar's output goes once it no longer has a console.
//!
//! Hiding the console window (SIDE-01) would otherwise throw away the only
//! diagnostic channel the embedded runtime has — and that channel has already
//! paid for itself: it was the `stop: cancel task` line in `llama-server`'s
//! output that revealed the 5s client timeout killing every long answer
//! (AD-028). One file per run, one run of history.

use std::fs::{self, File};
use std::path::{Path, PathBuf};

const LOG_NAME: &str = "llama-server.log";
const PREVIOUS_LOG_NAME: &str = "llama-server.log.1";

/// Lives next to the llama.cpp binary itself, under the user's base folder.
pub fn log_path(base_path: &Path) -> PathBuf {
    base_path.join("runtime").join(LOG_NAME)
}

pub fn previous_log_path(base_path: &Path) -> PathBuf {
    base_path.join("runtime").join(PREVIOUS_LOG_NAME)
}

/// Opens a fresh log, keeping the previous run as `.log.1`.
///
/// Returns `None` rather than an error on every failure: a read-only folder or
/// a full disk is a reason to lose the log, never a reason to refuse to start
/// the AI engine (SIDE-11).
pub fn open_rotating(base_path: &Path) -> Option<File> {
    let current = log_path(base_path);
    if let Some(parent) = current.parent() {
        fs::create_dir_all(parent).ok()?;
    }

    if current.exists() {
        // A rename that fails (the previous process still holds the handle on
        // Windows) is not worth giving up the new log for — `File::create`
        // truncates and the run is recorded either way.
        let _ = fs::rename(&current, previous_log_path(base_path));
    }

    File::create(&current).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("localmind-log-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_log_lives_beside_the_binary_it_belongs_to() {
        let base = Path::new("/data/localmind");
        assert_eq!(log_path(base), base.join("runtime").join("llama-server.log"));
        assert_eq!(
            previous_log_path(base),
            base.join("runtime").join("llama-server.log.1")
        );
    }

    #[test]
    fn a_second_run_pushes_the_first_one_aside() {
        use std::io::Write;

        let base = temp_dir("rotate");
        let mut first = open_rotating(&base).unwrap();
        first.write_all(b"execucao anterior").unwrap();
        drop(first);

        let second = open_rotating(&base).unwrap();
        drop(second);

        assert_eq!(
            fs::read_to_string(previous_log_path(&base)).unwrap(),
            "execucao anterior"
        );
        assert_eq!(
            fs::read_to_string(log_path(&base)).unwrap(),
            "",
            "the new run starts on an empty file"
        );

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn only_one_generation_is_kept() {
        use std::io::Write;

        let base = temp_dir("generations");
        for run in 1..=3 {
            let mut file = open_rotating(&base).unwrap();
            write!(file, "run {run}").unwrap();
        }
        // The third run rotated the second one out; the first is gone.
        assert_eq!(fs::read_to_string(previous_log_path(&base)).unwrap(), "run 2");
        assert_eq!(fs::read_to_string(log_path(&base)).unwrap(), "run 3");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn the_runtime_folder_is_created_when_missing() {
        let base = temp_dir("nofolder");
        assert!(open_rotating(&base).is_some());
        assert!(base.join("runtime").is_dir());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn an_unusable_folder_gives_up_the_log_not_the_sidecar() {
        // A path that cannot be created at all: the caller gets None and starts
        // the process anyway.
        let unusable = Path::new("\0invalid");
        assert!(open_rotating(unusable).is_none());
    }
}
