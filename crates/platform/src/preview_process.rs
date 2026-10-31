// Naygo — supervisor de lectores no cooperativos; sólo se invoca desde workers.
// Copyright (c) 2026 Nicolás Groth <ngroth@gmail.com>. ISGroth.
// SPDX-License-Identifier: MIT

use naygo_core::{
    preview_wire::{self, Response},
    CancellationToken,
};
use std::{
    io,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

pub const ARGUMENT: &str = "--naygo-preview-reader-v1";
pub const ANIMATION_ARGUMENT: &str = "--naygo-preview-animation-v1";
#[path = "preview_animation_process.rs"]
mod animation;
pub use animation::run as run_animation;
pub const WALL_LIMIT: Duration = Duration::from_secs(5);
pub const MEMORY_LIMIT: usize = 192 * 1024 * 1024;

/// Called by the internal dispatcher only; suppresses OS fault dialogs, not user UI.
pub fn silent_reader_errors() {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Diagnostics::Debug::*;
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
    }
}

/// Verify containment after reading the bounded request, before parsing a file. A manually
/// launched internal mode without a supervisor must fail closed too.
pub fn reader_is_contained() -> bool {
    reader_contained(false)
}

pub fn animation_reader_is_contained() -> bool {
    reader_contained(true)
}

fn reader_contained(animation: bool) -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::System::JobObjects::*;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        let valid = unsafe {
            QueryInformationJobObject(
                None,
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as _,
                std::mem::size_of_val(&info) as u32,
                None,
            )
        }
        .is_ok();
        let required = JOB_OBJECT_LIMIT_PROCESS_MEMORY
            | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
            | JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let cpu_valid = if animation {
            let mut rate = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION::default();
            unsafe {
                QueryInformationJobObject(
                    None,
                    JobObjectCpuRateControlInformation,
                    &mut rate as *mut _ as _,
                    std::mem::size_of_val(&rate) as u32,
                    None,
                )
                .is_ok()
                    && rate.ControlFlags
                        == (JOB_OBJECT_CPU_RATE_CONTROL_ENABLE
                            | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP)
                    && rate.Anonymous.CpuRate > 0
                    && rate.Anonymous.CpuRate <= 2000
            }
        } else {
            info.BasicLimitInformation
                .LimitFlags
                .contains(JOB_OBJECT_LIMIT_PROCESS_TIME)
                && info.BasicLimitInformation.PerProcessUserTimeLimit > 0
                && info.BasicLimitInformation.PerProcessUserTimeLimit <= 50_000_000
        };
        valid
            && cpu_valid
            && (info.BasicLimitInformation.LimitFlags & required) == required
            && info.ProcessMemoryLimit > 0
            && info.ProcessMemoryLimit <= MEMORY_LIMIT
            && info.BasicLimitInformation.ActiveProcessLimit == 1
    }
    #[cfg(not(windows))]
    {
        let _ = animation;
        false
    }
}

#[derive(Debug)]
pub enum Error {
    Cancelled,
    Timeout,
    Failed,
}

/// Owns the child through confirmed exit, including every error path. Killing/reaping and
/// pipe joins occur on this supervisor worker, NEVER on the UI thread. Parent-owned Windows
/// Job kills the child if Naygo exits/crashes; no inherited job handle and no descendants.
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[derive(Debug)]
pub struct Output {
    pub response: Response,
    pub peak_committed_bytes: usize,
}

pub fn run(
    executable: &Path,
    request: Vec<u8>,
    token: &CancellationToken,
) -> Result<Output, Error> {
    run_with_limit(executable, &[ARGUMENT], request, token, WALL_LIMIT)
}

fn run_with_limit(
    executable: &Path,
    arguments: &[&str],
    request: Vec<u8>,
    token: &CancellationToken,
    limit: Duration,
) -> Result<Output, Error> {
    if token.is_cancelled() {
        return Err(Error::Cancelled);
    }
    if request.is_empty() || request.len() > preview_wire::REQUEST_MAX {
        return Err(Error::Failed);
    }
    let started = Instant::now();
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .env("RAYON_NUM_THREADS", "1") // Decoder pools must not compete with navigation on every core.
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS. Preview is subordinate to navigation.
        command.creation_flags(0x08000000 | 0x00004000);
    }
    let mut process = Process(command.spawn().map_err(|_| Error::Failed)?);
    // The child blocks reading its request until AFTER the limits are applied. Failure to
    // attach is fail-closed, not a silent fallback to unbounded in-process decoding.
    let job = Job::attach(&process.0).map_err(|_| Error::Failed)?;
    let mut input = process.0.stdin.take().ok_or(Error::Failed)?;
    // Geometry uses small scalar fields: buffer the pipe, not a second whole scene.
    let mut output =
        io::BufReader::with_capacity(64 * 1024, process.0.stdout.take().ok_or(Error::Failed)?);
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let pipe = std::thread::Builder::new()
        .name("naygo-preview-pipe".into())
        .spawn(move || {
            let result = (|| {
                preview_wire::write_request(&mut input, &request)?;
                drop(input);
                let response = preview_wire::read_response(&mut output)?;
                // Exactly one response and EOF. Truncated/extra output is not accepted as success.
                use std::io::Read;
                let mut extra = [0];
                if output.read(&mut extra)? != 0 {
                    return Err(io::Error::other("extra preview output"));
                }
                Ok(response)
            })();
            let _ = tx.send(result);
        })
        .map_err(|_| Error::Failed)?;
    let result = loop {
        if token.is_cancelled() {
            break Err(Error::Cancelled);
        }
        if started.elapsed() >= limit {
            break Err(Error::Timeout);
        }
        match rx.recv_timeout(Duration::from_millis(25)) {
            Ok(Ok(response)) => {
                // EOF precedes process exit by a short interval: still honor cancellation/deadline.
                loop {
                    match process.0.try_wait() {
                        Ok(Some(_)) => break,
                        Ok(None) if !token.is_cancelled() && started.elapsed() < limit => {
                            std::thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                        _ => break,
                    }
                }
                if token.is_cancelled() {
                    break Err(Error::Cancelled);
                }
                if started.elapsed() >= limit {
                    break Err(Error::Timeout);
                }
                break match process.0.try_wait() {
                    Ok(Some(status)) if status.success() => Ok(Output {
                        response,
                        peak_committed_bytes: job.peak(),
                    }),
                    _ => Err(Error::Failed),
                };
            }
            Ok(Err(_)) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break Err(Error::Failed)
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
    };
    // Kill before join: neither a full stdin/stdout pipe nor a stuck parser can postpone Cancel.
    drop(process);
    #[cfg(windows)]
    drop(job);
    let _ = pipe.join();
    result
}

#[cfg(windows)]
struct Job(windows::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Job {
    fn attach(child: &Child) -> io::Result<Self> {
        Self::attach_mode(child, false)
    }
    fn attach_mode(child: &Child, animation: bool) -> io::Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::{Foundation::HANDLE, System::JobObjects::*};
        // SAFETY: owned job handle, valid borrowed child handle, initialized limits and exact sizes.
        unsafe {
            let job = Self(CreateJobObjectW(None, None).map_err(io::Error::other)?);
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
                | JOB_OBJECT_LIMIT_PROCESS_MEMORY
                | JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
            if !animation {
                info.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_PROCESS_TIME;
            }
            info.BasicLimitInformation.ActiveProcessLimit = 1;
            info.BasicLimitInformation.PerProcessUserTimeLimit = 5 * 10_000_000; // 100-ns units.
            info.ProcessMemoryLimit = MEMORY_LIMIT;
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                &info as *const _ as _,
                std::mem::size_of_val(&info) as u32,
            )
            .map_err(io::Error::other)?;
            if animation {
                // No cumulative CPU/wall cutoff for valid loops or pauses. Bound CPU rate instead;
                // the supervisor still kills a requested frame after five seconds without progress.
                let rate = JOBOBJECT_CPU_RATE_CONTROL_INFORMATION {
                    ControlFlags: JOB_OBJECT_CPU_RATE_CONTROL_ENABLE
                        | JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP,
                    Anonymous: JOBOBJECT_CPU_RATE_CONTROL_INFORMATION_0 { CpuRate: 2000 },
                };
                SetInformationJobObject(
                    job.0,
                    JobObjectCpuRateControlInformation,
                    &rate as *const _ as _,
                    std::mem::size_of_val(&rate) as u32,
                )
                .map_err(io::Error::other)?;
            }
            AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle()))
                .map_err(io::Error::other)?;
            Ok(job)
        }
    }
    fn peak(&self) -> usize {
        use windows::Win32::System::JobObjects::*;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        unsafe {
            QueryInformationJobObject(
                Some(self.0),
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as _,
                std::mem::size_of_val(&info) as u32,
                None,
            )
        }
        .map(|_| info.PeakProcessMemoryUsed)
        .unwrap_or(0)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    fn powershell() -> std::path::PathBuf {
        std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe")
    }
    fn probe(
        script: &str,
        bytes: Vec<u8>,
        token: &CancellationToken,
        limit: Duration,
    ) -> Result<Output, Error> {
        run_with_limit(
            &powershell(),
            &[
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ],
            bytes,
            token,
            limit,
        )
    }
    #[test]
    fn timeout_kills_reader_even_with_full_stdin_pipe() {
        let started = Instant::now();
        let result = probe(
            "Start-Sleep -Seconds 30",
            vec![0; preview_wire::REQUEST_MAX],
            &CancellationToken::new(),
            Duration::from_millis(350),
        );
        assert!(matches!(result, Err(Error::Timeout)), "{result:?}");
        assert!(started.elapsed() < Duration::from_secs(4));
    }
    #[test]
    fn cancellation_kills_blocked_reader_and_releases_pipe() {
        let token = CancellationToken::new();
        let cancel = token.clone();
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(250));
            cancel.cancel();
        });
        let started = Instant::now();
        let result = probe(
            "Start-Sleep -Seconds 30",
            vec![0; preview_wire::REQUEST_MAX],
            &token,
            WALL_LIMIT,
        );
        thread.join().unwrap();
        assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
        assert!(started.elapsed() < Duration::from_secs(4));
    }
    #[test]
    fn crash_and_truncated_response_fail_without_retry_loop() {
        for script in [
            "exit 7",
            "[Console]::OpenStandardOutput().Write([byte[]]@(78,71,80,49),0,4)",
        ] {
            let result = probe(script, vec![1], &CancellationToken::new(), WALL_LIMIT);
            assert!(matches!(result, Err(Error::Failed)), "{result:?}");
        }
    }
    #[test]
    fn job_limits_are_applied_and_closing_job_terminates_child() {
        use std::os::windows::process::CommandExt;
        use windows::Win32::System::JobObjects::*;
        let mut process = Process(
            Command::new(powershell())
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Start-Sleep -Seconds 30",
                ])
                .creation_flags(0x08000000)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let job = Job::attach(&process.0).unwrap();
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        unsafe {
            QueryInformationJobObject(
                Some(job.0),
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as _,
                std::mem::size_of_val(&info) as u32,
                None,
            )
        }
        .unwrap();
        assert_eq!(info.ProcessMemoryLimit, MEMORY_LIMIT);
        assert_eq!(info.BasicLimitInformation.ActiveProcessLimit, 1);
        assert_eq!(
            info.BasicLimitInformation.PerProcessUserTimeLimit,
            50_000_000
        );
        drop(job);
        let deadline = Instant::now() + Duration::from_secs(3);
        while process.0.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    #[test]
    fn rejects_cancelled_or_oversized_requests_without_spawning() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(matches!(
            run(Path::new("missing.exe"), vec![1], &token),
            Err(Error::Cancelled)
        ));
        assert!(matches!(
            run(
                Path::new("missing.exe"),
                vec![0; preview_wire::REQUEST_MAX + 1],
                &CancellationToken::new()
            ),
            Err(Error::Failed)
        ));
    }

    #[test]
    fn memory_limit_rejects_large_allocation_in_real_child() {
        // A valid response is emitted ONLY when allocation fails under the job limit.
        // No baseline allocates 384 MiB without containment and no user file is touched.
        let script = "try { $a = [byte[]]::new(402653184); exit 9 } catch { [Console]::OpenStandardOutput().Write([byte[]]@(78,71,80,49,4,0,0,0,0,0,0,0,0,7,0,0,0,108,105,109,105,116,101,100),0,24) }";
        let output = probe(script, vec![1], &CancellationToken::new(), WALL_LIMIT).unwrap();
        assert_eq!(output.response, Response::Message("limited".into()));
        assert!(output.peak_committed_bytes > 0 && output.peak_committed_bytes <= MEMORY_LIMIT);
    }

    #[test]
    fn cpu_budget_terminates_busy_reader_before_wall_deadline() {
        let started = Instant::now();
        let result = probe(
            "while ($true) {}",
            vec![1],
            &CancellationToken::new(),
            Duration::from_secs(20),
        );
        assert!(matches!(result, Err(Error::Failed)), "{result:?}");
        assert!(started.elapsed() < Duration::from_secs(20));
    }
}
#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(not(windows))]
struct Job;
#[cfg(not(windows))]
impl Job {
    // Naygo's isolated decoder currently requires Windows resource containment.
    fn attach_mode(child: &Child, _: bool) -> io::Result<Self> {
        Self::attach(child)
    }
    fn attach(_: &Child) -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows preview job required",
        ))
    }
    fn peak(&self) -> usize {
        0
    }
}
