use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceProbe {
    /// A device llama.cpp can actually offload to, named as the binary
    /// reported it (e.g. "NVIDIA GeForce RTX 2060").
    GpuAvailable(String),
    /// The binary ran fine but listed no usable device — same binary, CPU.
    CpuOnly,
    /// The binary itself could not run (missing Vulkan loader, bad build):
    /// a different answer from "no GPU", because it means downloading the
    /// CPU-only asset instead of just passing `-ngl 0`.
    BinaryFailed(String),
}

/// The binary is its own detector: it already knows which devices llama.cpp
/// can use, which a GPU crate could not answer (it would report "Vulkan
/// exists", not "llama.cpp can use it") — AD-022.
pub fn probe_devices(binary: &Path) -> DeviceProbe {
    let mut command = Command::new(binary);
    command.arg("--list-devices");
    // Suppresses the console that used to flash for an instant on Windows.
    // `CREATE_NO_WINDOW` hides the window, not the pipes — `output()` still
    // captures stdout, which is where the device list comes from.
    super::process::configure_command(&mut command);
    let output = command.output();

    match output {
        Ok(out) => classify_output(
            &String::from_utf8_lossy(&out.stdout),
            &String::from_utf8_lossy(&out.stderr),
            out.status.success(),
        ),
        Err(e) => DeviceProbe::BinaryFailed(e.to_string()),
    }
}

/// Kept pure so the three outcomes are testable without a real binary.
pub fn classify_output(stdout: &str, stderr: &str, exit_ok: bool) -> DeviceProbe {
    if !exit_ok {
        let reason = if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        };
        return DeviceProbe::BinaryFailed(reason);
    }

    for line in stdout.lines().chain(stderr.lines()) {
        let line = line.trim();
        // Device lines look like: `Vulkan0: NVIDIA GeForce RTX 2060 (6144 MiB, 5136 MiB free)`
        let Some((label, rest)) = line.split_once(':') else {
            continue;
        };
        if !label.starts_with("Vulkan") {
            continue;
        }
        let name = rest
            .split('(')
            .next()
            .unwrap_or(rest)
            .trim()
            .to_string();
        if !name.is_empty() {
            return DeviceProbe::GpuAvailable(name);
        }
    }

    DeviceProbe::CpuOnly
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_a_vulkan_device() {
        let stdout = "Available devices:\n  Vulkan0: NVIDIA GeForce RTX 2060 (6144 MiB, 5136 MiB free)\n";

        assert_eq!(
            classify_output(stdout, "", true),
            DeviceProbe::GpuAvailable("NVIDIA GeForce RTX 2060".to_string())
        );
    }

    #[test]
    fn no_vulkan_device_means_cpu_only() {
        let stdout = "Available devices:\n";

        assert_eq!(classify_output(stdout, "", true), DeviceProbe::CpuOnly);
    }

    #[test]
    fn a_failing_exit_code_reports_the_binary_as_broken() {
        let stderr = "error while loading shared libraries: libvulkan.so.1: cannot open shared object file";

        let probe = classify_output("", stderr, false);

        assert!(matches!(probe, DeviceProbe::BinaryFailed(msg) if msg.contains("libvulkan")));
    }

    #[test]
    fn a_missing_binary_reports_binary_failed() {
        let probe = probe_devices(Path::new("./definitely-not-a-real-llama-server"));

        assert!(matches!(probe, DeviceProbe::BinaryFailed(_)));
    }
}

/// Runs the real binary, with the real flags, to answer the one question no
/// unit test can: does suppressing the console also suppress the output we
/// parse? If it did, GPU detection would return nothing and the app would fall
/// back to CPU **silently** — the worst possible outcome of hiding the window.
///
/// Run with:
///   set LOCALMIND_LLAMA_SERVER=<path to llama-server.exe> && cargo test detect_real -- --ignored --nocapture
#[cfg(test)]
mod detect_real {
    use super::*;

    #[test]
    #[ignore = "needs LOCALMIND_LLAMA_SERVER pointing at a real binary"]
    fn hiding_the_console_does_not_hide_the_device_list() {
        let Ok(path) = std::env::var("LOCALMIND_LLAMA_SERVER") else {
            panic!("set LOCALMIND_LLAMA_SERVER to a real llama-server binary");
        };

        let probe = probe_devices(Path::new(&path));
        println!("probe: {probe:?}");

        match probe {
            DeviceProbe::GpuAvailable(name) => {
                assert!(!name.is_empty(), "a device was found but came back nameless");
                println!("GPU detectada com o console suprimido: {name}");
            }
            DeviceProbe::CpuOnly => {
                panic!("no device listed — this is the silent CPU fallback the flag must not cause")
            }
            DeviceProbe::BinaryFailed(reason) => panic!("the binary could not run: {reason}"),
        }
    }
}
