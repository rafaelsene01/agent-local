//! Reads the updater manifest (`latest.json`) published on the GitHub Release.
//!
//! The same file feeds both update paths. `tauri-plugin-updater` looks up the
//! key for the installer format it was built as; the portable path looks up
//! `windows-x86_64-portable`, which the release workflow appends after
//! `tauri-action` has written the file. `platforms` is a plain map, so the extra
//! key is inert for the plugin.

use serde::Deserialize;
use std::collections::HashMap;

use super::InstallFlavor;

/// Added by `scripts/patch-latest-json.mjs` in the release workflow.
pub const PORTABLE_WINDOWS_KEY: &str = "windows-x86_64-portable";

#[derive(Debug, Clone, Deserialize)]
pub struct Manifest {
    pub version: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub pub_date: Option<String>,
    pub platforms: HashMap<String, PlatformEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlatformEntry {
    pub url: String,
    pub signature: String,
}

/// Which manifest key this build should download.
///
/// `None` for installed builds — those go through `tauri-plugin-updater`, which
/// resolves its own key. `None` for a portable build on a platform we do not
/// ship a portable bundle for (Linux uses the AppImage, which the official
/// updater already replaces in place without root).
pub fn platform_key(flavor: InstallFlavor) -> Option<&'static str> {
    match flavor {
        InstallFlavor::Portable if cfg!(target_os = "windows") => Some(PORTABLE_WINDOWS_KEY),
        _ => None,
    }
}

/// A missing key is "nothing to download for you", not an error: a release that
/// happens to lack the portable artifact must not surface as a failure.
pub fn select<'a>(manifest: &'a Manifest, key: &str) -> Option<&'a PlatformEntry> {
    manifest.platforms.get(key)
}

pub async fn fetch(url: &str) -> Result<Manifest, String> {
    let response = crate::providers::http_client()
        .get(url)
        .timeout(crate::providers::SHORT_REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| format!("não foi possível consultar atualizações: {e}"))?;

    if !response.status().is_success() {
        return Err(format!(
            "servidor de atualização respondeu {}",
            response.status()
        ));
    }

    let body = response
        .text()
        .await
        .map_err(|e| format!("resposta de atualização ilegível: {e}"))?;

    parse(&body)
}

pub fn parse(body: &str) -> Result<Manifest, String> {
    serde_json::from_str(body).map_err(|e| format!("manifesto de atualização inválido: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "version": "1.2.3",
        "notes": "linha de notas",
        "pub_date": "2026-07-26T12:00:00Z",
        "platforms": {
            "windows-x86_64-nsis": { "signature": "sig-nsis", "url": "https://e/setup.exe" },
            "windows-x86_64-portable": { "signature": "sig-zip", "url": "https://e/portable.zip" },
            "linux-x86_64-appimage": { "signature": "sig-app", "url": "https://e/app.AppImage" }
        }
    }"#;

    #[test]
    fn parses_a_full_manifest() {
        let manifest = parse(SAMPLE).unwrap();
        assert_eq!(manifest.version, "1.2.3");
        assert_eq!(manifest.notes.as_deref(), Some("linha de notas"));
        assert_eq!(manifest.pub_date.as_deref(), Some("2026-07-26T12:00:00Z"));
        assert_eq!(manifest.platforms.len(), 3);
    }

    #[test]
    fn notes_and_date_are_optional() {
        let manifest = parse(r#"{"version":"1.0.0","platforms":{}}"#).unwrap();
        assert_eq!(manifest.version, "1.0.0");
        assert!(manifest.notes.is_none());
        assert!(manifest.pub_date.is_none());
    }

    #[test]
    fn unknown_platform_keys_do_not_break_the_parse() {
        // Mirrors the assumption the design depends on from the other side: an
        // extra key must be inert, never a parse failure.
        let body = r#"{
            "version": "1.0.0",
            "platforms": {
                "windows-x86_64-portable": { "signature": "s", "url": "u" },
                "something-we-invent-later": { "signature": "s", "url": "u" }
            }
        }"#;
        let manifest = parse(body).unwrap();
        assert!(select(&manifest, PORTABLE_WINDOWS_KEY).is_some());
    }

    #[test]
    fn select_returns_the_matching_entry() {
        let manifest = parse(SAMPLE).unwrap();
        let entry = select(&manifest, PORTABLE_WINDOWS_KEY).unwrap();
        assert_eq!(entry.url, "https://e/portable.zip");
        assert_eq!(entry.signature, "sig-zip");
    }

    #[test]
    fn a_missing_platform_is_none_not_an_error() {
        let manifest = parse(r#"{"version":"1.0.0","platforms":{}}"#).unwrap();
        assert!(select(&manifest, PORTABLE_WINDOWS_KEY).is_none());
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse("not json").is_err());
        assert!(parse(r#"{"platforms":{}}"#).is_err(), "version is required");
    }

    #[test]
    fn installed_builds_have_no_key_of_their_own() {
        assert!(platform_key(InstallFlavor::Installed).is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn portable_windows_maps_to_the_portable_key() {
        assert_eq!(
            platform_key(InstallFlavor::Portable),
            Some(PORTABLE_WINDOWS_KEY)
        );
    }
}
