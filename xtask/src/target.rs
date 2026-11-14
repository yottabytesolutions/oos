//! The scope build: `cargo check` for aarch64-unknown-linux-musl.
//!
//! On the host with the target's standard library (`rustup target add aarch64-unknown-linux-musl`): `check` does not
//! link, so no cross linker is needed. Its own target directory keeps it apart from the host build. The earlier
//! Docker flow mounted every worktree at the same path and could take another tree's build as fresh.

use crate::repo::TARGET;
use crate::run;
use std::path::Path;

/// Checks every target of the crate in `root/krate` for the scope.
pub fn check(root: &Path, krate: &str) -> Result<(), String> {
    let dir = root.join("target").join("xtask-arm64");
    let dir = dir.to_string_lossy();
    let out = run::capture(
        &root.join(krate),
        "cargo",
        &["check", "--all-targets", "--quiet", "--target", TARGET, "--target-dir", &dir],
    )?;
    if out.ok {
        Ok(())
    } else {
        Err(run::tail(&out.stderr, 30))
    }
}

/// Whether the target's standard library is installed.
pub fn installed() -> bool {
    run::capture(Path::new("."), "rustup", &["target", "list", "--installed"])
        .is_ok_and(|o| o.stdout.lines().any(|l| l.trim() == TARGET))
}
