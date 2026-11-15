//! Running external commands with their output captured.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Immutable per-run environment and log destination. Default commands inherit their environment.
#[derive(Debug, Default)]
pub(crate) struct Context {
    target: Option<PathBuf>,
    logs: Option<PathBuf>,
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
    let mut command = Command::new(program);
    command.args(args).current_dir(dir);
    if let Some(target) = &ctx.target {
        command.env("CARGO_TARGET_DIR", target);
    }
    if let Some(logs) = &ctx.logs {
        command.env("OOS_CHECK_LOG_DIR", logs);
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
    Ok((Context { target: Some(selected), logs: Some(logs.clone()) }, logs))
}

/// Whether `program` can be started (a `--version` call succeeds).
pub fn available(ctx: &Context, program: &str, args: &[&str]) -> bool {
    capture(ctx, Path::new("."), program, args).is_ok_and(|o| o.ok)
}

/// The last `n` lines of `text`, for a failure report.
pub fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

#[cfg(test)]
mod tests;
