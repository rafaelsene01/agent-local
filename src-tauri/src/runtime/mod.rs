pub mod release;

/// Only these two are shipped as prebuilt llama.cpp binaries by the project
/// for the platforms this milestone targets; anything else makes the embedded
/// runtime unavailable instead of downloading something guaranteed to fail.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TargetOs {
    Windows,
    Linux,
}

impl TargetOs {
    pub fn current() -> Option<Self> {
        match std::env::consts::OS {
            "windows" => Some(TargetOs::Windows),
            "linux" => Some(TargetOs::Linux),
            _ => None,
        }
    }
}

/// Vulkan covers NVIDIA/AMD/Intel with a single binary and no vendor toolkit;
/// CUDA is deliberately out of scope (AD-022). Cpu is the fallback for
/// machines where the Vulkan build cannot even start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Backend {
    Vulkan,
    Cpu,
}

impl Backend {
    pub fn as_str(&self) -> &'static str {
        match self {
            Backend::Vulkan => "vulkan",
            Backend::Cpu => "cpu",
        }
    }
}

#[derive(Debug)]
pub enum RuntimeError {
    UnsupportedPlatform,
    Network(String),
    AssetNotFound(String),
    Io(String),
}

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::UnsupportedPlatform => write!(
                f,
                "runtime embutido não está disponível neste sistema operacional"
            ),
            RuntimeError::Network(msg) => write!(f, "falha de rede: {msg}"),
            RuntimeError::AssetNotFound(name) => {
                write!(f, "o release do llama.cpp não contém o arquivo '{name}'")
            }
            RuntimeError::Io(msg) => write!(f, "erro de arquivo: {msg}"),
        }
    }
}

impl std::error::Error for RuntimeError {}
