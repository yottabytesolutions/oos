// SPDX-License-Identifier: GPL-2.0-or-later
//! Bounded data cleanup and recovery through the initramfs shutdown helper.

use std::io;
#[cfg(any(target_os = "linux", test))]
use std::process::Output;
#[cfg(target_os = "linux")]
use std::process::{Command, Stdio};
#[cfg(any(target_os = "linux", test))]
use std::time::Duration;

/// The mount to recover if the power or restart request returns.
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub struct DataShutdown {
    recovery: io::Result<Option<Recovery>>,
}

#[cfg(target_os = "linux")]
impl DataShutdown {
    /// Close write admission and attempt to unmount after UI teardown.
    pub fn prepare() -> Self {
        let result = prepare_helper(
            || helper(&["ui-drained"]),
            || {
                if let Err(error) = bounded_sync(Duration::from_secs(2), || {
                    let status = Command::new("/bin/sync")
                        .args(["-f", "/oos"])
                        .stdin(Stdio::null())
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()?;
                    if status.success() {
                        Ok(())
                    } else {
                        Err(io::Error::other(status.to_string()))
                    }
                }) {
                    log(&format!("fallback sync of /oos failed: {error}"));
                }
            },
        );
        if let Err(error) = helper_status(result.as_ref()) {
            log(&format!("data cleanup failed: {error}; continuing shutdown"));
        }
        let recovery = result.and_then(|output| match output.status.code() {
            Some(73) => Ok(None), // The helper never acquired write admission.
            Some(code @ (0 | 1 | 124)) => Recovery::parse(&output.stdout, code == 124).map(Some),
            _ => Err(io::Error::other(output.status.to_string())),
        });
        Self { recovery }
    }

    /// Restore and verify /oos before resuming normal use. Append any restore error to the original failure.
    pub fn restore(self, failure: String) -> String {
        let result = self.recovery.and_then(|recovery| {
            let Some(recovery) = recovery else { return Ok(()) };
            let worker = recovery.worker.map(|pid| pid.to_string()).unwrap_or_default();
            let mut args = vec!["restore", worker.as_str()];
            if let Some(mount) = &recovery.mount {
                args.extend(mount.iter().map(String::as_str));
            }
            helper_status(helper(&args).as_ref())
        });
        report_restore(failure, result, |message| write_notice("/tmp/oos-data.notice", message))
    }
}

#[cfg(any(target_os = "linux", test))]
fn prepare_helper(run: impl FnOnce() -> io::Result<Output>, sync: impl FnOnce()) -> io::Result<Output> {
    let result = run();
    if result.is_err() {
        sync();
    }
    result
}

#[cfg(any(target_os = "linux", test))]
fn bounded_sync(seconds: Duration, sync: impl FnOnce() -> io::Result<()> + Send + 'static) -> io::Result<()> {
    let (done, result) = std::sync::mpsc::channel();
    std::thread::Builder::new().name("oos-shutdown-sync".into()).spawn(move || {
        let _ = done.send(sync());
    })?;
    // Dropping the handle keeps the caller independent of a sync stuck in the kernel.
    result.recv_timeout(seconds).map_err(|error| match error {
        std::sync::mpsc::RecvTimeoutError::Timeout => io::Error::new(io::ErrorKind::TimedOut, error),
        std::sync::mpsc::RecvTimeoutError::Disconnected => io::Error::other(error),
    })?
}

#[derive(Debug, PartialEq, Eq)]
struct Recovery {
    worker: Option<i32>,
    mount: Option<[String; 3]>,
}

impl Recovery {
    fn parse(bytes: &[u8], timed_out: bool) -> io::Result<Self> {
        let text = std::str::from_utf8(bytes).map_err(io::Error::other)?;
        let mut recovery = Self { worker: None, mount: None };
        for line in text.lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            match fields.as_slice() {
                ["worker", pid] if timed_out && recovery.worker.is_none() => {
                    recovery.worker = Some(parse_worker(pid)?);
                }
                ["mount", source, filesystem, options] if recovery.mount.is_none() => {
                    recovery.mount = Some([source, filesystem, options].map(|field| (*field).to_owned()));
                }
                _ => return Err(io::Error::other("invalid shutdown recovery information")),
            }
        }
        if timed_out && recovery.worker.is_none() {
            return Err(io::Error::other("missing timed-out shutdown worker PID"));
        }
        Ok(recovery)
    }
}

#[cfg(target_os = "linux")]
fn helper(args: &[&str]) -> io::Result<Output> {
    Command::new("/bin/oos-shutdown-data").args(args).current_dir("/").stdin(Stdio::null()).output()
}

#[cfg(any(target_os = "linux", test))]
fn helper_status(result: Result<&Output, &io::Error>) -> io::Result<()> {
    match result {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(io::Error::other(format!("oos-shutdown-data: {}", output.status))),
        Err(error) => Err(io::Error::other(format!("start oos-shutdown-data: {error}"))),
    }
}

fn parse_worker(pid: &str) -> io::Result<i32> {
    pid.parse::<i32>()
        .ok()
        .filter(|&pid| pid > 1)
        .ok_or_else(|| io::Error::other("invalid timed-out shutdown worker PID"))
}

fn report_restore(failure: String, result: io::Result<()>, notice: impl FnOnce(&str)) -> String {
    match result {
        Ok(()) => failure,
        Err(error) => {
            let reason = format!("restore /oos failed: {error}");
            notice(&format!("The data area is unavailable for saving settings. {reason}. Restart to retry."));
            format!("{failure}; {reason}")
        }
    }
}

#[cfg(target_os = "linux")]
fn log(message: &str) {
    use std::io::Write;
    let line = format!("oos-shutdown-data: {message}\n");
    for path in ["/dev/kmsg", "/proc/self/fd/2"] {
        if std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .and_then(|mut file| file.write_all(line.as_bytes()))
            .is_ok()
        {
            return;
        }
    }
}

#[cfg(target_os = "linux")]
fn write_notice(path: &str, message: &str) {
    log(message);
    if let Err(error) = std::fs::write(path, message) {
        log(&format!("could not write {path}: {error}"));
    }
}

/// Show a power failure on the running or restarted UI, and log it.
#[cfg(target_os = "linux")]
pub fn report_power_failure(message: &str) {
    write_notice("/tmp/oos-power.notice", message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_mount_and_pending_worker_for_recovery() {
        let data = Recovery::parse(b"worker 123\nmount /dev/example ext4 rw,noatime\n", true).unwrap();
        assert_eq!(data.worker, Some(123));
        assert_eq!(data.mount.unwrap(), ["/dev/example", "ext4", "rw,noatime"]);
        let unmounted = Recovery::parse(b"", false).unwrap();
        assert!(unmounted.mount.is_none());
        assert!(unmounted.worker.is_none());
    }

    #[test]
    fn helper_diagnostic_distinguishes_spawn_error_from_nonzero_exit() {
        let error = io::Error::new(io::ErrorKind::NotFound, "helper missing");
        assert_eq!(helper_status(Err(&error)).unwrap_err().to_string(), "start oos-shutdown-data: helper missing");
        let output = std::process::Command::new("sh").args(["-c", "exit 7"]).output().unwrap();
        let error = helper_status(Ok(&output)).unwrap_err().to_string();
        assert!(error.contains(&output.status.to_string()), "{error}");
    }

    #[test]
    fn only_spawn_failure_uses_the_fallback_sync() {
        let synced = std::cell::Cell::new(false);
        let result = prepare_helper(|| Err(io::ErrorKind::NotFound.into()), || synced.set(true));
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
        assert!(synced.get());
        let result = prepare_helper(
            || std::process::Command::new("sh").args(["-c", "exit 73"]).output(),
            || panic!("helper has already attempted sync"),
        );
        assert_eq!(result.unwrap().status.code(), Some(73));
    }

    #[test]
    fn sync_deadline_does_not_join_a_blocked_worker() {
        let (release, blocked) = std::sync::mpsc::channel();
        let (started, running) = std::sync::mpsc::channel();
        let result = bounded_sync(Duration::ZERO, move || {
            started.send(()).unwrap();
            blocked.recv().unwrap();
            Ok(())
        });
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        running.recv().unwrap();
        release.send(()).unwrap();
    }

    #[test]
    fn timeout_receipt_identifies_only_a_valid_process_group() {
        for invalid in ["0", "1", "-2", "", "1\n2", "4294967295"] {
            assert!(Recovery::parse(format!("worker {invalid}\n").as_bytes(), true).is_err());
        }
        assert!(Recovery::parse(b"", true).is_err());
    }

    #[test]
    fn failed_restore_names_both_errors_and_publishes_unavailable_notice() {
        let mut written = String::new();
        let error = report_restore("reboot returned".into(), Err(io::Error::other("mount refused")), |notice| {
            written = notice.into();
        });
        assert_eq!(error, "reboot returned; restore /oos failed: mount refused");
        assert!(written.contains("data area is unavailable"));
        assert!(written.contains("mount refused"));
        assert_eq!(report_restore("returned".into(), Ok(()), |_| panic!("data is writable")), "returned");
    }
}
