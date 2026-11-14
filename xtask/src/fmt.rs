//! Formatting: `cargo fmt --check` with the root rustfmt.toml.

use crate::run;
use std::path::Path;

/// The changed lines `cargo fmt` would make in the crate in `root/krate`.
pub fn diff_lines(root: &Path, krate: &str) -> Result<usize, String> {
    let out = run::capture(&root.join(krate), "cargo", &["fmt", "--", "--check", "--color", "never"])?;
    if !out.ok && out.stdout.is_empty() {
        return Err(run::tail(&out.stderr, 10));
    }
    Ok(count(&out.stdout))
}

/// Changed lines of a `cargo fmt --check` diff.
fn count(diff: &str) -> usize {
    diff.lines().filter(|l| l.starts_with('+') || l.starts_with('-')).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_changed_lines() {
        assert_eq!(count("Diff in a.rs:1:\n-a\n+b\n+c\n keep\n"), 3);
        assert_eq!(count(""), 0);
    }
}
