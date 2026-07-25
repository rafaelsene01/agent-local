use super::{Backend, RuntimeError, TargetOs};
use serde::Deserialize;
use std::time::Duration;

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/ggml-org/llama.cpp/releases/latest";

/// api.github.com rejects requests without a User-Agent.
const USER_AGENT: &str = "localmind-app";

#[derive(Debug, Deserialize, Clone)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Release {
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

/// llama.cpp has no long-lived stable version — tags are incrementing build
/// numbers (`b10107`), so the release is resolved at download time and the
/// resolved tag is what gets persisted (EMBED-14).
pub async fn resolve_latest() -> Result<Release, RuntimeError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| RuntimeError::Network(e.to_string()))?;

    let resp = client
        .get(LATEST_RELEASE_URL)
        .header("User-Agent", USER_AGENT)
        .send()
        .await
        .map_err(|e| RuntimeError::Network(e.to_string()))?;

    if !resp.status().is_success() {
        // 403 here is almost always the unauthenticated 60 req/h rate limit,
        // which is worth naming instead of showing a bare status code.
        return Err(RuntimeError::Network(format!(
            "GitHub respondeu {} ao resolver o release do llama.cpp (a API permite 60 requisições por hora sem autenticação)",
            resp.status()
        )));
    }

    resp.json::<Release>()
        .await
        .map_err(|e| RuntimeError::Network(e.to_string()))
}

/// The suffix a release asset must end with for this OS/backend pair.
pub fn asset_suffix(os: TargetOs, backend: Backend) -> &'static str {
    match (os, backend) {
        (TargetOs::Windows, Backend::Vulkan) => "-bin-win-vulkan-x64.zip",
        (TargetOs::Windows, Backend::Cpu) => "-bin-win-cpu-x64.zip",
        (TargetOs::Linux, Backend::Vulkan) => "-bin-ubuntu-vulkan-x64.tar.gz",
        (TargetOs::Linux, Backend::Cpu) => "-bin-ubuntu-x64.tar.gz",
    }
}

/// Matches on the exact suffix, never "contains vulkan": the same release
/// also publishes `win-cuda-12.4-x64.zip`, `win-hip-radeon-x64.zip` and
/// `ubuntu-vulkan-arm64.tar.gz`, and a loose match picks the wrong file.
pub fn pick_asset<'a>(assets: &'a [Asset], os: TargetOs, backend: Backend) -> Option<&'a Asset> {
    let suffix = asset_suffix(os, backend);
    assets.iter().find(|a| a.name.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Asset names as published in release `b10107` (confirmed live during
    /// design, see design.md Research Findings).
    fn b10107_assets() -> Vec<Asset> {
        [
            "llama-b10107-bin-win-cpu-x64.zip",
            "llama-b10107-bin-win-cuda-12.4-x64.zip",
            "llama-b10107-bin-win-cuda-13.3-x64.zip",
            "llama-b10107-bin-win-vulkan-x64.zip",
            "llama-b10107-bin-win-hip-radeon-x64.zip",
            "llama-b10107-bin-ubuntu-x64.tar.gz",
            "llama-b10107-bin-ubuntu-vulkan-x64.tar.gz",
            "llama-b10107-bin-ubuntu-vulkan-arm64.tar.gz",
        ]
        .iter()
        .map(|name| Asset {
            name: name.to_string(),
            browser_download_url: format!("https://example.invalid/{name}"),
            size: 1,
        })
        .collect()
    }

    #[test]
    fn picks_the_windows_vulkan_build() {
        let assets = b10107_assets();
        let picked = pick_asset(&assets, TargetOs::Windows, Backend::Vulkan).unwrap();
        assert_eq!(picked.name, "llama-b10107-bin-win-vulkan-x64.zip");
    }

    #[test]
    fn picks_the_windows_cpu_build() {
        let assets = b10107_assets();
        let picked = pick_asset(&assets, TargetOs::Windows, Backend::Cpu).unwrap();
        assert_eq!(picked.name, "llama-b10107-bin-win-cpu-x64.zip");
    }

    #[test]
    fn picks_the_linux_builds_without_grabbing_arm64() {
        let assets = b10107_assets();
        let vulkan = pick_asset(&assets, TargetOs::Linux, Backend::Vulkan).unwrap();
        assert_eq!(vulkan.name, "llama-b10107-bin-ubuntu-vulkan-x64.tar.gz");

        let cpu = pick_asset(&assets, TargetOs::Linux, Backend::Cpu).unwrap();
        assert_eq!(cpu.name, "llama-b10107-bin-ubuntu-x64.tar.gz");
    }

    #[test]
    fn never_falls_back_to_a_cuda_or_hip_build() {
        let assets: Vec<Asset> = b10107_assets()
            .into_iter()
            .filter(|a| !a.name.contains("vulkan"))
            .collect();

        let picked = pick_asset(&assets, TargetOs::Windows, Backend::Vulkan);

        assert!(picked.is_none(), "a missing asset must not resolve to a guess");
    }

    #[test]
    fn missing_asset_returns_none() {
        assert!(pick_asset(&[], TargetOs::Linux, Backend::Cpu).is_none());
    }
}
