// SPDX-License-Identifier: GPL-2.0-or-later

use std::{
    fs::{self, DirBuilder, File},
    io::{self, Read, Seek},
    os::unix::{
        fs::{DirBuilderExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn new(target: &Path) -> Result<Self, String> {
        let mut suffix = 0;
        loop {
            let path = target.join(format!("unit-bench-{}-{suffix}", std::process::id()));
            match DirBuilder::new().mode(0o700).create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => suffix += 1,
                Err(_) => return Err("create unit-bench scratch directory".into()),
            }
        }
    }

    pub(super) fn file(&self, name: &str) -> io::Result<File> {
        let path = self.0.join(name);
        let file = File::options().create_new(true).read(true).write(true).mode(0o600).open(&path)?;
        fs::remove_file(path)?;
        Ok(file)
    }

    #[cfg(test)]
    pub(super) fn for_test() -> Self {
        let target = crate::repo::root().join("target");
        fs::create_dir_all(&target).unwrap();
        Self::new(&target).unwrap()
    }

    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Scratch cleanup is best-effort once the command's result is known.
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) enum Stderr {
    Discard,
    OnFailure,
}

pub(super) fn output(
    command: &mut Command,
    scratch: &Scratch,
    operation: &str,
    seconds: u64,
    diagnostics: Stderr,
) -> Result<String, String> {
    // A file keeps the wait independent of pipe capacity and inherited write ends.
    let mut stdout = scratch.file("stdout").map_err(|_| format!("{operation}: open command output"))?;
    let sink = stdout.try_clone().map_err(|_| format!("{operation}: duplicate command output"))?;
    let mut stderr = match diagnostics {
        Stderr::Discard => None,
        Stderr::OnFailure => Some(scratch.file("stderr").map_err(|_| format!("{operation}: open command stderr"))?),
    };
    let stderr_sink = match &stderr {
        Some(file) => file.try_clone().map_err(|_| format!("{operation}: duplicate command stderr"))?.into(),
        None => Stdio::null(),
    };
    complete(command.stdout(sink).stderr(stderr_sink), operation, seconds).map_err(|error| {
        match stderr.as_mut().and_then(failure_detail) {
            Some(detail) => format!("{error}; stderr: {detail}"),
            None => error,
        }
    })?;
    let mut text = String::new();
    stdout
        .rewind()
        .and_then(|()| stdout.read_to_string(&mut text))
        .map_err(|_| format!("{operation}: read command output"))?;
    Ok(text)
}

fn failure_detail(stderr: &mut File) -> Option<String> {
    const LIMIT: usize = 2048;
    stderr.rewind().ok()?;
    let mut bytes = Vec::new();
    stderr.take((LIMIT + 1) as u64).read_to_end(&mut bytes).ok()?;
    let truncated = bytes.len() > LIMIT;
    bytes.truncate(LIMIT);
    let text = String::from_utf8_lossy(&bytes);
    let detail: String = text.trim().chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    if truncated {
        Some(format!("{detail} [truncated]"))
    } else {
        (!detail.is_empty()).then_some(detail)
    }
}

pub(super) fn complete(command: &mut Command, operation: &str, seconds: u64) -> Result<(), String> {
    let timeout = Duration::from_secs(seconds);
    let mut child = command.process_group(0).spawn().map_err(|_| format!("{operation}: could not start command"))?;
    match wait(&mut child, timeout) {
        Ok(Some(status)) if status.success() => Ok(()),
        Ok(Some(status)) => Err(format!("{operation}: command exited with {status}")),
        outcome => {
            terminate(&mut child);
            let error = if outcome.is_ok() { "timed out" } else { "could not wait for command" };
            Err(format!("{operation}: {error}"))
        }
    }
}

fn wait(child: &mut Child, timeout: Duration) -> io::Result<Option<ExitStatus>> {
    let start = Instant::now();
    loop {
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Ok(None);
        }
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        thread::sleep(remaining.min(Duration::from_millis(20)));
    }
}

fn terminate(child: &mut Child) {
    // The unreaped child still owns the group ID. Include local SSH helpers in the kill.
    // If the group kill fails, still attempt to kill and reap the direct child.
    let _ = Command::new("/bin/kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    // The group kill may already have terminated the child.
    let _ = child.kill();
    // The caller already has the timeout or wait error and cannot salvage the command output.
    let _ = child.wait();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_command_names_only_the_operation() {
        let error = complete(
            Command::new("/bin/sh")
                .args(["-c", "printf '%s' example.invalid >&2; read line"])
                .stdin(Stdio::null())
                .stderr(Stdio::null()),
            "resolve SSH destination",
            0,
        )
        .unwrap_err();
        assert_eq!(error, "resolve SSH destination: timed out");
    }

    #[test]
    fn failed_cache_command_includes_bounded_stderr() {
        let scratch = Scratch::for_test();
        let error = output(
            Command::new("/bin/sh").args(["-c", "printf 'cache denied %4096s omitted' x >&2; exit 7"]),
            &scratch,
            "check fixture cache",
            15,
            Stderr::OnFailure,
        )
        .unwrap_err();
        assert!(error.starts_with("check fixture cache: command exited with exit status: 7; stderr: cache denied"));
        assert!(error.ends_with("[truncated]"));
        assert!(!error.contains("omitted"));
    }
}
