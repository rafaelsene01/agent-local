//! Update support (M8).
//!
//! Two delivery flavors share one trust root and one UI:
//!
//! * **Installed** (`.msi` / NSIS / `.AppImage`) — handled by the official
//!   `tauri-plugin-updater`.
//! * **Portable** (a zip the user unpacks anywhere) — handled by
//!   [`portable`], because the official updater has no portable target on
//!   Windows. See `.specs/features/release-distribution/design.md`.
//!
//! Which one we are is decided by a marker file, never by inspecting the path:
//! an NSIS `currentUser` install lands in `%LOCALAPPDATA%` and a portable copy
//! can be unpacked literally anywhere, including inside `Program Files`, so no
//! path tells the two apart reliably.

pub mod manifest;
pub mod portable;
pub mod signature;

use serde::Serialize;
use std::path::{Path, PathBuf};

/// Written into the portable zip by `scripts/make-portable.mjs`. The two sides
/// must agree; the script has a test asserting this exact string.
pub const PORTABLE_MARKER: &str = ".portable";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallFlavor {
    Installed,
    Portable,
}

/// Directory holding the running executable — the folder a portable update
/// replaces the contents of.
pub fn app_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

pub fn flavor() -> InstallFlavor {
    flavor_at(app_dir().as_deref())
}

/// Split from [`flavor`] so it can be tested without faking `current_exe()`.
/// Anything unknown resolves to `Installed`: that path is the conservative one,
/// since it never rewrites files next to the executable.
pub fn flavor_at(dir: Option<&Path>) -> InstallFlavor {
    match dir {
        Some(dir) if dir.join(PORTABLE_MARKER).exists() => InstallFlavor::Portable,
        _ => InstallFlavor::Installed,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
    pub pub_date: Option<String>,
    pub flavor: InstallFlavor,
}

/// Numeric `major.minor.patch` comparison.
///
/// String comparison is wrong here in a way that only bites late: `"0.1.10"`
/// sorts *before* `"0.1.9"` lexicographically, so the tenth patch release would
/// silently stop offering itself.
pub fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse_version(candidate), parse_version(current)) {
        (Some(new), Some(old)) => new > old,
        _ => false,
    }
}

fn parse_version(raw: &str) -> Option<(u64, u64, u64)> {
    let trimmed = raw.trim();
    let trimmed = trimmed.strip_prefix('v').unwrap_or(trimmed);
    // Ignore any pre-release/build suffix; the release pipeline never produces
    // one today, but a stray "-beta" must not be read as a malformed version.
    let core = trimmed
        .split(['-', '+'])
        .next()
        .unwrap_or(trimmed);

    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_patch_is_newer() {
        assert!(is_newer("0.1.2", "0.1.1"));
        assert!(is_newer("0.2.0", "0.1.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
    }

    #[test]
    fn double_digit_patch_beats_single_digit() {
        // The whole reason this is not a string comparison.
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.9", "0.1.10"));
    }

    #[test]
    fn same_or_older_is_not_newer() {
        assert!(!is_newer("1.2.3", "1.2.3"));
        assert!(!is_newer("1.2.2", "1.2.3"));
        assert!(!is_newer("0.9.9", "1.0.0"));
    }

    #[test]
    fn malformed_versions_never_offer_an_update() {
        assert!(!is_newer("", "1.0.0"));
        assert!(!is_newer("1.0", "1.0.0"));
        assert!(!is_newer("next", "1.0.0"));
        assert!(!is_newer("1.0.0.1", "1.0.0"));
        assert!(!is_newer("2.0.0", "not-a-version"));
    }

    #[test]
    fn tag_prefix_and_prerelease_suffix_are_tolerated() {
        assert!(is_newer("v1.2.4", "1.2.3"));
        assert!(is_newer("1.2.4-beta.1", "1.2.3"));
        assert!(!is_newer("1.2.3-beta.1", "1.2.3"));
    }

    #[test]
    fn flavor_follows_the_marker_file() {
        let dir = std::env::temp_dir().join(format!("localmind-flavor-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(flavor_at(Some(&dir)), InstallFlavor::Installed);

        std::fs::write(dir.join(PORTABLE_MARKER), "").unwrap();
        assert_eq!(flavor_at(Some(&dir)), InstallFlavor::Portable);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn unknown_location_falls_back_to_installed() {
        assert_eq!(flavor_at(None), InstallFlavor::Installed);
        assert_eq!(
            flavor_at(Some(Path::new("/definitely/not/a/real/path"))),
            InstallFlavor::Installed
        );
    }
}
