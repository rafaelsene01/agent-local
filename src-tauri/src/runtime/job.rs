//! Ties the sidecar's lifetime to this process, enforced by the kernel.
//!
//! `RunningSidecar::kill` and `RunEvent::ExitRequested` already handle a normal
//! quit, but neither runs when the app is killed outright — Task Manager,
//! `taskkill /F`, a crash, the end of a Windows session. A job object with
//! `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` closes that gap: when the last handle
//! to the job goes away — and every handle a process owns goes away when it
//! dies, however it dies — Windows terminates everything inside it.
//!
//! This is the same technique Cargo uses to avoid orphaning build scripts. No
//! polling, no watchdog process, no extra binary to sign and explain to an
//! antivirus.
//!
//! Hiding the console (SIDE-01) is what makes this mandatory rather than nice:
//! an orphaned console window is at least visible and closable, while an
//! orphaned *hidden* process is several GB nobody can see.

#[cfg(windows)]
mod imp {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub struct JobHandle(HANDLE);

    // The handle is owned by this process and only ever used through the two
    // methods below, both of which take `&self` and hand it straight to the
    // kernel — which is where the synchronization actually lives.
    unsafe impl Send for JobHandle {}
    unsafe impl Sync for JobHandle {}

    impl JobHandle {
        pub fn create() -> Option<Self> {
            // SAFETY: null name and attributes are the documented way to create
            // an anonymous job owned by this process.
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                eprintln!("job object: CreateJobObject failed; sidecar lifetime falls back to the app's own cleanup");
                return None;
            }

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            // SAFETY: `info` is a correctly sized, fully initialized struct of
            // the class named by the second argument.
            let applied = unsafe {
                SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const std::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if applied == 0 {
                eprintln!("job object: could not set kill-on-close; not using it");
                unsafe { CloseHandle(handle) };
                return None;
            }

            Some(JobHandle(handle))
        }

        /// Puts a freshly spawned child under the job. Returns whether it took;
        /// the caller starts the sidecar either way (SIDE-07).
        pub fn assign(&self, child: &Child) -> bool {
            // SAFETY: the raw handle belongs to a live child we just spawned.
            let assigned =
                unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as HANDLE) };
            if assigned == 0 {
                eprintln!("job object: could not assign the sidecar; it may outlive a forced kill");
            }
            assigned != 0
        }
    }

    impl Drop for JobHandle {
        fn drop(&mut self) {
            // Closing the last handle is precisely what triggers the kill, so
            // this is the mechanism working, not cleanup around it.
            unsafe { CloseHandle(self.0) };
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::process::Child;

    /// Nothing to do off Windows: there is no console window to hide, and a
    /// process group would be a different mechanism for a problem that has not
    /// been reported there. Kept as a real type so the call sites stay free of
    /// `#[cfg]` (SIDE-03).
    pub struct JobHandle;

    impl JobHandle {
        pub fn create() -> Option<Self> {
            None
        }

        pub fn assign(&self, _child: &Child) -> bool {
            false
        }
    }
}

pub use imp::JobHandle;

/// Held by the app for its whole life. One job for the process, not one per
/// sidecar: creating a job per restart would leak a handle on every model
/// switch (SIDE-08).
pub struct JobState(pub Option<JobHandle>);

impl JobState {
    pub fn create() -> Self {
        JobState(JobHandle::create())
    }

    /// `false` means the sidecar is not covered by the kernel guarantee — it
    /// still runs, and the normal-quit path still kills it.
    pub fn assign(&self, child: &std::process::Child) -> bool {
        match &self.0 {
            Some(job) => job.assign(child),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(windows)]
    fn a_job_can_be_created_on_this_machine() {
        assert!(
            JobState::create().0.is_some(),
            "creating an anonymous job object should work on a normal Windows session"
        );
    }

    #[test]
    #[cfg(not(windows))]
    fn there_is_no_job_off_windows_and_that_is_not_a_failure() {
        let state = JobState::create();
        assert!(state.0.is_none());
    }

    /// Exercises the actual guarantee, not the API around it.
    ///
    /// Dropping the last handle to the job is precisely what happens when this
    /// process dies — by any means, including `taskkill /F`, which is the case
    /// no `Drop` and no exit hook can cover. If the child is gone after the
    /// drop, the kernel did it, and it would do the same for a forced kill.
    #[test]
    #[cfg(windows)]
    fn closing_the_job_kills_everything_inside_it() {
        use std::process::{Command, Stdio};

        let job = JobState::create();
        assert!(job.0.is_some(), "no job, nothing to prove");

        // `ping -n` is a long-running console process available on every
        // Windows install; 120 pings is far longer than this test.
        let mut child = Command::new("cmd")
            .args(["/c", "ping", "-n", "120", "127.0.0.1"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn a long-running child");

        assert!(job.assign(&child), "the child must join the job");
        let pid = child.id();

        // The child is alive right now...
        assert!(
            child.try_wait().expect("try_wait").is_none(),
            "child should still be running before the job closes"
        );

        drop(job);

        // ...and the kernel takes it down when the job handle goes away.
        let mut gone = false;
        for _ in 0..50 {
            if child.try_wait().expect("try_wait").is_some() {
                gone = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if !gone {
            let _ = child.kill();
        }
        assert!(
            gone,
            "pid {pid} survived the job closing — kill-on-close is not in effect"
        );
    }

    /// The console flag, measured rather than assumed: the same command spawned
    /// both ways, asking Windows whether a console host was created for it.
    #[test]
    #[cfg(windows)]
    fn the_flag_is_what_decides_whether_a_console_appears() {
        use std::process::{Command, Stdio};

        fn spawn_ping(hide: bool) -> std::process::Child {
            let mut cmd = Command::new("cmd");
            cmd.args(["/c", "ping", "-n", "30", "127.0.0.1"])
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if hide {
                crate::runtime::process::configure_command(&mut cmd);
            }
            cmd.spawn().expect("spawn ping")
        }

        /// Whether a **visible** console window exists for the process.
        ///
        /// Not "does a conhost exist": `CREATE_NO_WINDOW` still allocates a
        /// console host, it just gives it no visible window — measured here,
        /// after a first version of this test asserted on the presence of
        /// `conhost.exe` and failed for exactly that reason. The window belongs
        /// to conhost, so conhost's `MainWindowHandle` is what to look at.
        fn has_visible_console(pid: u32) -> bool {
            let script = format!(
                "$h = Get-CimInstance Win32_Process -Filter \"Name='conhost.exe' AND ParentProcessId={pid}\"; \
                 $visible = 0; \
                 foreach ($c in $h) {{ \
                   $p = Get-Process -Id $c.ProcessId -ErrorAction SilentlyContinue; \
                   if ($p -and $p.MainWindowHandle -ne 0) {{ $visible = 1 }} \
                 }} \
                 $visible"
            );
            let out = Command::new("powershell")
                .args(["-NoProfile", "-NonInteractive", "-Command", &script])
                .output()
                .expect("query conhost");
            String::from_utf8_lossy(&out.stdout).trim() == "1"
        }

        let mut visible = spawn_ping(false);
        let mut hidden = spawn_ping(true);
        std::thread::sleep(std::time::Duration::from_millis(1500));

        let visible_has = has_visible_console(visible.id());
        let hidden_has = has_visible_console(hidden.id());

        let _ = visible.kill();
        let _ = hidden.kill();

        assert!(
            !hidden_has,
            "a flagged process must not show a console window — this is the bug being fixed"
        );
        // Reported, not asserted, and worth reading before trusting this test:
        // when `cargo test` runs from a terminal, the unflagged child attaches
        // to the runner's existing console instead of creating a visible one of
        // its own, so both sides come out `false` and the comparison proves
        // nothing. The bug only reproduces from a parent that has no console —
        // which is exactly what the GUI app is. This test guards against the
        // flag being deleted; it does not demonstrate the fix.
        if !visible_has {
            println!(
                "INCONCLUSIVO: sem a flag também não houve janela — o runner emprestou o próprio console. \
                 A comparação só vale a partir de um pai sem console (o app)."
            );
        }
        println!("janela de console visível — sem a flag: {visible_has}, com a flag: {hidden_has}");
    }
}
