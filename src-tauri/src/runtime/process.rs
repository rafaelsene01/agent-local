use super::job::JobState;
use super::RuntimeError;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

/// `CREATE_NO_WINDOW`. `llama-server.exe` is a console application, so without
/// this Windows hands it a console window of its own — which is the black
/// terminal that appeared next to the app. Documented as ignored for
/// non-console executables, which is why it is harmless to apply blindly.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The only place that knows about console windows. Keeping it here is what
/// keeps `#[cfg]` out of the spawn flow itself (SIDE-03).
#[cfg(windows)]
pub fn configure_command(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn configure_command(_cmd: &mut Command) {}

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
    /// The user's base folder, when there is one. The sidecar's output is
    /// written under it; without a folder there is simply no log.
    pub base_path: Option<PathBuf>,
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

pub async fn spawn(cfg: SidecarConfig, job: &JobState) -> Result<RunningSidecar, RuntimeError> {
    let mut command = Command::new(&cfg.binary);
    command.args(build_args(&cfg));
    configure_command(&mut command);

    // With no console there is nowhere for the output to go, so it goes to a
    // file — or to nowhere, if the folder will not cooperate (SIDE-11).
    match cfg.base_path.as_deref().and_then(super::log::open_rotating) {
        Some(file) => {
            let errors = file
                .try_clone()
                .map_err(|e| RuntimeError::Io(format!("erro de arquivo: {e}")))?;
            command.stdout(Stdio::from(file)).stderr(Stdio::from(errors));
        }
        None => {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
    }

    let child = command
        .spawn()
        .map_err(|e| RuntimeError::Io(format!("não foi possível iniciar o llama-server: {e}")))?;

    // Right after spawn and before the health check: the earlier it joins the
    // job, the smaller the window in which a forced kill could orphan it.
    job.assign(&child);

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
            base_path: None,
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

/// The whole sidecar path against the real binary and the real model: hidden
/// console, output captured to the rotating log, and the process joined to a
/// job that takes it down when the handle closes.
///
/// This is the closest a test can get to T7 without a human looking at a screen
/// — and unlike the app, it needs no connection to be active and touches no
/// configuration.
///
/// Run with:
///   set LOCALMIND_LLAMA_SERVER=... && set LOCALMIND_GGUF=... && set LOCALMIND_BASE=...
///   cargo test sidecar_real -- --ignored --nocapture
#[cfg(test)]
mod sidecar_real {
    use super::*;

    #[tokio::test]
    #[ignore = "starts a real llama-server and loads a real model"]
    async fn the_real_sidecar_is_hidden_logged_and_dies_with_the_job() {
        let binary = std::env::var("LOCALMIND_LLAMA_SERVER").expect("set LOCALMIND_LLAMA_SERVER");
        let model = std::env::var("LOCALMIND_GGUF").expect("set LOCALMIND_GGUF");
        let base = PathBuf::from(std::env::var("LOCALMIND_BASE").expect("set LOCALMIND_BASE"));

        let job = JobState::create();
        println!("job criado: {}", job.0.is_some());

        let cfg = SidecarConfig {
            binary: PathBuf::from(&binary),
            model: PathBuf::from(&model),
            port: free_port().unwrap(),
            context_length: Some(2048),
            gpu_layers: -1,
            base_path: Some(base.clone()),
        };
        let port = cfg.port;

        let mut sidecar = spawn(cfg, &job).await.expect("the sidecar should start");
        println!("sidecar respondeu ao health check em 127.0.0.1:{port}");

        // The output has to be somewhere now that there is no console.
        let log = super::super::log::log_path(&base);
        let contents = std::fs::read_to_string(&log).expect("the log must exist");
        assert!(
            !contents.trim().is_empty(),
            "the log is empty — the output went nowhere"
        );
        println!("log com {} bytes em {}", contents.len(), log.display());

        let pid = sidecar.child.id();
        drop(job);

        // Closing the job is what a forced kill of the app does to its handles.
        let mut gone = false;
        for _ in 0..100 {
            if sidecar.child.try_wait().expect("try_wait").is_some() {
                gone = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if !gone {
            sidecar.kill();
        }
        assert!(gone, "llama-server pid {pid} survived the job closing");
        println!("llama-server pid {pid} encerrado pelo kernel ao fechar o job");
    }
}
