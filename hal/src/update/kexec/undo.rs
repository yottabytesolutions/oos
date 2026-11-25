// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Best-effort kexec undo: report each failure and let the caller continue cleanup.

use std::fmt::Display;
use std::io::Write;
use std::process::{Command, Stdio};

// The caller's terminal may be gone (telnet dropped by the quiesce, a closed pipe): stderr writes must not panic,
// or panic = abort would stop the undo before the links and the UI are restored. A failed stderr write is noted in
// the kernel and persistent logs instead.
fn stderr(line: &str) -> std::io::Result<()> {
    std::io::stderr().lock().write_all(line.as_bytes())
}

pub(super) fn report(message: &str) {
    let mut line = format!("oos-kexec: {message}\n");
    if let Err(e) = stderr(&line) {
        line = format!("oos-kexec: {message} (stderr: {e})\n");
    }
    // pmsg is ramoops-backed (CONFIG_PSTORE_PMSG), including after /oos becomes read only.
    for name in ["/dev/kmsg", "/dev/pmsg0"] {
        if let Err(e) =
            std::fs::OpenOptions::new().write(true).open(name).and_then(|mut f| f.write_all(line.as_bytes()))
        {
            // the terminal is the only channel left; if it is gone too, the message is lost by necessity
            if stderr(&format!("oos-kexec: {name} diagnostic write failed: {e}\n")).is_err() {
                continue;
            }
        }
    }
}

/// Report an undo failure to stderr and kernel/persistent logs without stopping subsequent attempts.
pub fn warn<T, E: Display>(action: &str, result: Result<T, E>) {
    warn_with(action, result, report);
}

fn warn_with<T, E: Display>(action: &str, result: Result<T, E>, log: impl FnOnce(&str)) {
    if let Err(error) = result {
        log(&format!("undo {action} failed: {error}; continuing"));
    }
}

fn command(command: &mut Command) -> Result<(), String> {
    let status = command.status().map_err(|error| error.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(status.to_string())
    }
}

/// Run one `ip` restore command; launch errors and nonzero exit status are reported.
pub fn ip(action: &str, args: &[&str]) {
    warn(action, command(Command::new("ip").args(args)));
}

/// Remove a marker file; one that is already gone is not a failure.
pub fn remove_marker(path: &str) {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        r => warn(&format!("remove {path}"), r),
    }
}

/// Restart the UI supervisor with its existing shell guard after restoring devices.
pub fn resume_ui(script: &str) {
    warn(
        "restart UI supervisor",
        Command::new("setsid")
            .args(["sh", "-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failures_report_the_operation_and_error_and_allow_later_attempts() {
        let mut messages = Vec::new();
        for (action, result) in [
            ("unload", Err("unload error")),
            ("remount", Err("remount error")),
            ("clear state", Err("state error")),
            ("restart UI", Err("spawn error")),
            ("success", Ok(())),
        ] {
            warn_with(action, result, |message| messages.push(message.to_string()));
        }
        assert_eq!(
            messages,
            [
                "undo unload failed: unload error; continuing",
                "undo remount failed: remount error; continuing",
                "undo clear state failed: state error; continuing",
                "undo restart UI failed: spawn error; continuing",
            ]
        );
    }

    #[test]
    fn command_failure_includes_launch_errors_and_nonzero_exit_status() {
        assert!(command(Command::new("sh").args(["-c", "exit 0"])).is_ok());
        let error = command(Command::new("sh").args(["-c", "exit 7"])).unwrap_err();
        assert!(error.contains('7'), "{error}");
        assert!(command(&mut Command::new("oos-nonexistent-undo-command")).is_err());
    }
}
