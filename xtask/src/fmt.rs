//! Formatting: `cargo fmt --check` with the root rustfmt.toml.

use crate::run;
use std::path::Path;

/// The changed lines `cargo fmt` would make in the crate in `root/krate`.
pub fn diff_lines(root: &Path, krate: &str) -> Result<usize, String> {
    let out = run::capture(&root.join(krate), "cargo", &["fmt", "--", "--check", "--color", "never"])?;
    verdict(out.ok, &out.stdout, &out.stderr)
}

/// Any failed run fails, whatever it printed; the diff only says how much differs.
fn verdict(ok: bool, stdout: &str, stderr: &str) -> Result<usize, String> {
    match (ok, count(stdout)) {
        (true, _) => Ok(0),
        (false, 0) => Err(format!("cargo fmt failed: {}{}", run::tail(stdout, 10), run::tail(stderr, 10))),
        (false, n) => Ok(n),
    }
}

/// Changed lines of a `cargo fmt --check` diff.
fn count(diff: &str) -> usize {
    diff.lines().filter(|l| l.starts_with('+') || l.starts_with('-')).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    // The hole: a failed run whose output had no diff lines passed as formatted.
    #[test]
    fn a_failed_run_never_passes() {
        assert_eq!(verdict(true, "", ""), Ok(0));
        assert_eq!(verdict(false, "Diff in a.rs:\n-a\n+b\n", ""), Ok(2));
        assert!(verdict(false, "error: something\n", "").is_err());
        assert!(verdict(false, "", "rustfmt crashed").is_err());
    }

    #[test]
    fn counts_changed_lines() {
        assert_eq!(count("Diff in a.rs:1:\n-a\n+b\n+c\n keep\n"), 3);
        assert_eq!(count(""), 0);
    }
}
