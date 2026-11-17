// SPDX-License-Identifier: GPL-2.0-or-later
//! Default Python and explicit image runners. Exit status decides success, never printed PASS.
use super::Outcome;
use std::path::Path;

pub(crate) fn command(ctx: &crate::run::Context, root: &Path, program: &str, args: &[&str]) -> Outcome {
    let out = crate::run::capture(ctx, root, program, args).map_err(|e| vec![e])?;
    let lines = vec![format!("{}{}", out.stdout, out.stderr)];
    if out.ok {
        Ok(lines)
    } else {
        Err(lines)
    }
}

pub(crate) fn doctor(ctx: &crate::run::Context, root: &Path) -> Outcome {
    crate::doctor::inventory(ctx, root, &["--gpu", "--sim"])
}

fn host(ctx: &crate::run::Context, root: &Path, id: &str) -> Outcome {
    command(ctx, root, "python3", &["-I", "tools/check_inventory.py", id])
}

pub(crate) fn python(ctx: &crate::run::Context, root: &Path) -> Outcome {
    host(ctx, root, "python-tests")
}
pub(crate) fn arm(ctx: &crate::run::Context, root: &Path) -> Outcome {
    host(ctx, root, "arm64-release-link")
}
pub(crate) fn ssh(ctx: &crate::run::Context, root: &Path) -> Outcome {
    host(ctx, root, "ssh-sim")
}
pub(crate) fn firewall(ctx: &crate::run::Context, root: &Path) -> Outcome {
    host(ctx, root, "firewall-sim")
}
pub(crate) fn gpu(ctx: &crate::run::Context, root: &Path) -> Outcome {
    command(ctx, root, "sh", &["gpu/test-host.sh"])
}
