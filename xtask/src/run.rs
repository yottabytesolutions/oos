//! Running external commands with their output captured.

use std::path::Path;
use std::process::Command;

/// The output of a finished command.
#[derive(Debug)]
pub struct Output {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `program args` in `dir` and captures its output. An error only when the program cannot start.
pub fn capture(dir: &Path, program: &str, args: &[&str]) -> Result<Output, String> {
    capture_env(dir, program, args, &[])
}

/// Capture a command with explicit environment overrides, retaining the same complete log.
pub(crate) fn capture_env(
    dir: &Path,
    program: &str,
    args: &[&str],
    env: &[(&str, &std::ffi::OsStr)],
) -> Result<Output, String> {
    let result = Command::new(program).args(args).envs(env.iter().copied()).current_dir(dir).output();
    if let Some(logs) = std::env::var_os("OOS_CHECK_LOG_DIR") {
        let stamp =
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
        let path = Path::new(&logs).join(format!("{}-{}.log", std::process::id(), stamp));
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
    let out = result.map_err(|e| format!("{} {}: {}", program, args.join(" "), e))?;
    Ok(Output {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Select output through the same worktree guard as every artifact builder.
pub(crate) fn start_logs(root: &Path) -> Result<std::path::PathBuf, String> {
    let output = capture(root, "python3", &["-I", "tools/output.py", "target"])?;
    if !output.ok {
        return Err(output.stderr);
    }
    let selected = std::path::PathBuf::from(output.stdout.trim());
    std::env::set_var("CARGO_TARGET_DIR", &selected);
    let stamp =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let logs = selected.join("check-logs").join(format!("{}-{}", std::process::id(), stamp));
    std::fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    std::env::set_var("OOS_CHECK_LOG_DIR", &logs);
    Ok(logs)
}

/// Whether `program` can be started (a `--version` call succeeds).
pub fn available(program: &str, args: &[&str]) -> bool {
    capture(Path::new("."), program, args).is_ok_and(|o| o.ok)
}

/// The last `n` lines of `text`, for a failure report.
pub fn tail(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(n)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_keeps_the_last_lines() {
        assert_eq!(tail("a\nb\nc", 2), "b\nc");
        assert_eq!(tail("a", 5), "a");
        assert_eq!(tail("", 3), "");
    }
}
