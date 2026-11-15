//! The scope's target: clippy checks the scope crates for aarch64-unknown-linux-musl on the host (`clippy::run`).
//! It needs the target's standard library only: clippy does not link. The flags come from the root
//! `.cargo/config.toml`, as for the image build.

use crate::repo::TARGET;
use crate::run;
use std::path::Path;

/// Whether the target's standard library is installed.
pub fn installed(ctx: &crate::run::Context) -> bool {
    run::capture(ctx, Path::new("."), "rustup", &["target", "list", "--installed"])
        .is_ok_and(|o| o.stdout.lines().any(|l| l.trim() == TARGET))
}
