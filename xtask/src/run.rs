// SPDX-License-Identifier: GPL-2.0-or-later
//! Running external commands with their output captured.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

type SourceMeasurements = Result<Vec<(crate::repo::Crate, crate::size::Measured)>, String>;

/// Immutable per-run environment and log destination. Default commands inherit their environment.
#[derive(Debug, Default)]
pub(crate) struct Context {
    target: Option<PathBuf>,
    logs: Option<PathBuf>,
    root: Option<PathBuf>,
    incremental: Option<std::ffi::OsString>,
    measurements: OnceLock<SourceMeasurements>,
    test_threads: Option<usize>,
}

impl Context {
    pub(crate) fn limit_test_threads(mut self, threads: Option<usize>) -> Self {
        self.test_threads = threads;
        self
    }

    /// Share one source walk among the ratchet checks of this run. Ad-hoc contexts always read fresh sources.
    pub(crate) fn source_measurements(
        &self,
        root: &Path,
        measure: impl FnOnce() -> SourceMeasurements,
    ) -> SourceMeasurements {
        if self.root.as_deref() == Some(root) {
            self.measurements.get_or_init(measure).clone()
        } else {
            measure()
        }
    }
}

/// The output of a finished command.
#[derive(Debug)]
pub struct Output {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `program args` in `dir` and captures its output. An error only when the program cannot start.
pub fn capture(ctx: &Context, dir: &Path, program: &str, args: &[&str]) -> Result<Output, String> {
    capture_env(ctx, dir, program, args, &[])
}

/// Capture a command with explicit environment overrides, retaining the same complete log.
pub(crate) fn capture_env(
    ctx: &Context,
    dir: &Path,
    program: &str,
    args: &[&str],
    env: &[(&str, &std::ffi::OsStr)],
) -> Result<Output, String> {
    let out = capture_raw(ctx, dir, program, args, env)?;
    Ok(Output {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Capture bytes for callers that must reject invalid UTF-8, with the same environment and logs.
pub(crate) fn capture_raw(
    ctx: &Context,
    dir: &Path,
    program: &str,
    args: &[&str],
    env: &[(&str, &std::ffi::OsStr)],
) -> Result<std::process::Output, String> {
    execute(ctx, dir, program, args, env, &mut Command::new(program))
}

/// Probe the repository's pinned toolchain instead of Cargo's inherited proxy override.
pub(crate) fn probe(ctx: &Context, root: &Path, program: &str, args: &[&str]) -> Result<std::process::Output, String> {
    execute(ctx, root, program, args, &[], Command::new(program).env_remove("RUSTUP_TOOLCHAIN"))
}

/// Variables with which Git selects a repository. Git exports them to its hooks, so a gate started from a hook would
/// pass them on: a child running `git` in a fixture repository would then write into the real clone. Every command
/// finds its repository from its working directory instead.
pub(crate) const GIT_REPOSITORY_VARS: [&str; 9] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_COMMON_DIR",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_PREFIX",
    "GIT_NAMESPACE",
    "GIT_QUARANTINE_PATH",
];

fn execute(
    ctx: &Context,
    dir: &Path,
    program: &str,
    args: &[&str],
    env: &[(&str, &std::ffi::OsStr)],
    command: &mut Command,
) -> Result<std::process::Output, String> {
    command.args(args).current_dir(dir);
    for name in GIT_REPOSITORY_VARS {
        command.env_remove(name);
    }
    if let Some(target) = &ctx.target {
        command.env("CARGO_TARGET_DIR", target);
    }
    if let Some(logs) = &ctx.logs {
        command.env("OOS_CHECK_LOG_DIR", logs);
    }
    if let Some(incremental) = &ctx.incremental {
        command.env("CARGO_INCREMENTAL", incremental);
    }
    if std::env::var_os("NEXTEST_TEST_THREADS").is_none() {
        if let Some(threads) = ctx.test_threads {
            command.env("NEXTEST_TEST_THREADS", threads.to_string());
        }
    }
    let result = command.envs(env.iter().copied()).output();
    if let Some(logs) = &ctx.logs {
        let stamp =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
        let path = logs.join(format!("{}-{}.log", std::process::id(), stamp));
        let detail = match &result {
            Ok(o) => format!(
                "{} {:?}\nstatus={}\nstdout:\n{}\nstderr:\n{}",
                program,
                args,
                o.status,
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ),
            Err(e) => format!("{} {:?}\nfailed to start: {}", program, args, e),
        };
        std::fs::write(path, detail).map_err(|e| format!("cannot retain command log: {}", e))?;
    }
    result.map_err(|e| format!("{} {}: {}", program, args.join(" "), e))
}

/// Select output through the same worktree guard as every artifact builder.
pub(crate) fn start_logs(root: &Path) -> Result<(Context, PathBuf), String> {
    let output = capture(&Context::default(), root, "python3", &["-I", "tools/output.py", "target"])?;
    if !output.ok {
        return Err(output.stderr);
    }
    let selected = PathBuf::from(output.stdout.trim());
    let stamp =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let logs = selected.join("check-logs").join(format!("{}-{}", std::process::id(), stamp));
    std::fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    Ok((
        Context {
            target: Some(selected),
            logs: Some(logs.clone()),
            root: Some(root.to_path_buf()),
            incremental: Some(std::env::var_os("CARGO_INCREMENTAL").unwrap_or_else(|| "0".into())),
            ..Default::default()
        },
        logs,
    ))
}

/// Whether `program` can be started (a `--version` call succeeds).
pub fn available(ctx: &Context, root: &Path, program: &str, args: &[&str]) -> bool {
    probe(ctx, root, program, args).is_ok_and(|o| o.status.success())
}

/// Parse an optional K/N work partition, rejecting invalid or out-of-range indices.
pub(crate) fn partition(name: &str, value: Option<&str>) -> Result<Option<(u32, u32)>, String> {
    let Some(value) = value.filter(|v| !v.is_empty()) else { return Ok(None) };
    let bad = || format!("{name}={value}: expected K/N with 1 <= K <= N");
    let (k, n) = value.split_once('/').ok_or_else(bad)?;
    let (k, n): (u32, u32) = (k.parse().map_err(|_| bad())?, n.parse().map_err(|_| bad())?);
    if k == 0 || k > n {
        return Err(bad());
    }
    Ok(Some((k, n)))
}

/// The last `n` lines of `text`, for a failure report.
pub fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

#[cfg(test)]
mod tests;
