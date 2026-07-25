use super::RuntimeError;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::time::Duration;

const HEALTH_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Loading a multi-GB model into memory takes a while on CPU; a short
/// deadline here would report a healthy sidecar as broken.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
pub struct SidecarConfig {
    pub binary: PathBuf,
    pub model: PathBuf,
    pub port: u16,
    pub context_length: Option<u32>,
    /// -1 offloads every layer to the GPU, 0 keeps everything on the CPU.
    pub gpu_layers: i32,
}

pub struct RunningSidecar {
    child: Child,
    pub port: u16,
}

impl RunningSidecar {
    pub fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// Idempotent: killing an already-dead child is a normal outcome (the
    /// app can quit after the sidecar crashed), not an error worth panicking.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for RunningSidecar {
    fn drop(&mut self) {
        self.kill();
    }
}

/// Same shape as `DbState`: a resource that may not exist yet.
pub struct SidecarState(pub Mutex<Option<RunningSidecar>>);

impl SidecarState {
    pub fn empty() -> Self {
        SidecarState(Mutex::new(None))
    }
}

/// Binds port 0, lets the OS assign a free port, then releases it. There is a
/// race between releasing and `llama-server` binding it; the alternative
/// (handing over the socket) needs support llama-server does not have, and a
/// lost race surfaces as a health-check timeout rather than silence.
pub fn free_port() -> Result<u16, RuntimeError> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| RuntimeError::Io(e.to_string()))?;
    let port = listener
        .local_addr()
        .map_err(|e| RuntimeError::Io(e.to_string()))?
        .port();
    Ok(port)
}

/// Pure so the flag mapping is testable without spawning anything.
pub fn build_args(cfg: &SidecarConfig) -> Vec<String> {
    let mut args = vec![
        "-m".to_string(),
        cfg.model.to_string_lossy().to_string(),
        "--host".to_string(),
        "127.0.0.1".to_string(),
        "--port".to_string(),
        cfg.port.to_string(),
        "-ngl".to_string(),
        cfg.gpu_layers.to_string(),
    ];
    // `-c` is omitted rather than passed as 0, so llama-server keeps its own
    // default of inheriting the context length from the model.
    if let Some(ctx) = cfg.context_length {
        args.push("-c".to_string());
        args.push(ctx.to_string());
    }
    args
}

pub async fn spawn(cfg: SidecarConfig) -> Result<RunningSidecar, RuntimeError> {
    let child = Command::new(&cfg.binary)
        .args(build_args(&cfg))
        .spawn()
        .map_err(|e| RuntimeError::Io(format!("não foi possível iniciar o llama-server: {e}")))?;

    let mut sidecar = RunningSidecar {
        child,
        port: cfg.port,
    };

    match wait_until_healthy(&mut sidecar).await {
        Ok(()) => Ok(sidecar),
        Err(e) => {
            sidecar.kill();
            Err(e)
        }
    }
}

/// Polls `/health` instead of assuming the process is usable the moment it
/// spawns: a crash during model load must surface as an error (EMBED-08),
/// not as a connection that hangs on the first message.
async fn wait_until_healthy(sidecar: &mut RunningSidecar) -> Result<(), RuntimeError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| RuntimeError::Network(e.to_string()))?;
    let url = format!("{}/health", sidecar.base_url());
    let deadline = std::time::Instant::now() + HEALTH_TIMEOUT;

    loop {
        if let Ok(Some(status)) = sidecar.child.try_wait() {
            return Err(RuntimeError::Io(format!(
                "o llama-server encerrou antes de ficar pronto ({status})"
            )));
        }

        if let Ok(resp) = client.get(&url).send().await {
            if resp.status().is_success() {
                return Ok(());
            }
        }

        if std::time::Instant::now() >= deadline {
            return Err(RuntimeError::Network(
                "o llama-server não respondeu ao health check a tempo".to_string(),
            ));
        }
        tokio::time::sleep(HEALTH_POLL_INTERVAL).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(context_length: Option<u32>, gpu_layers: i32) -> SidecarConfig {
        SidecarConfig {
            binary: PathBuf::from("llama-server"),
            model: PathBuf::from("/models/phi.gguf"),
            port: 1234,
            context_length,
            gpu_layers,
        }
    }

    #[test]
    fn free_port_returns_a_usable_port() {
        let port = free_port().unwrap();
        assert!(port > 0);
        // The port must be free again right after, otherwise llama-server
        // could never bind it.
        TcpListener::bind(("127.0.0.1", port)).expect("port should have been released");
    }

    #[test]
    fn gpu_layers_map_to_ngl() {
        let gpu = build_args(&cfg(None, -1));
        assert_eq!(gpu.windows(2).find(|w| w[0] == "-ngl").unwrap()[1], "-1");

        let cpu = build_args(&cfg(None, 0));
        assert_eq!(cpu.windows(2).find(|w| w[0] == "-ngl").unwrap()[1], "0");
    }

    #[test]
    fn context_length_is_omitted_when_not_configured() {
        assert!(!build_args(&cfg(None, 0)).contains(&"-c".to_string()));

        let args = build_args(&cfg(Some(8192), 0));
        assert_eq!(args.windows(2).find(|w| w[0] == "-c").unwrap()[1], "8192");
    }

    #[test]
    fn model_and_host_are_always_passed() {
        let args = build_args(&cfg(None, 0));
        assert_eq!(args.windows(2).find(|w| w[0] == "-m").unwrap()[1], "/models/phi.gguf");
        assert_eq!(args.windows(2).find(|w| w[0] == "--host").unwrap()[1], "127.0.0.1");
        assert_eq!(args.windows(2).find(|w| w[0] == "--port").unwrap()[1], "1234");
    }
}
