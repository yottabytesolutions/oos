// SPDX-License-Identifier: GPL-2.0-or-later
//! Default Python and explicit image runners. Exit status decides success, never printed PASS.
use super::Outcome;
use crate::run::Context;
use std::path::{Path, PathBuf};

#[cfg(test)]
pub(super) mod tests;

pub(crate) fn command(ctx: &Context, root: &Path, program: &str, args: &[&str]) -> Outcome {
    let out = crate::run::capture(ctx, root, program, args).map_err(|e| vec![e])?;
    let lines = vec![format!("{}{}", out.stdout, out.stderr)];
    if out.ok {
        Ok(lines)
    } else {
        Err(lines)
    }
}

pub(crate) fn target(ctx: &Context, root: &Path) -> Result<PathBuf, Vec<String>> {
    let out = crate::run::capture(ctx, root, "python3", &["-I", "tools/output.py", "target"]).map_err(|e| vec![e])?;
    if !out.ok {
        return Err(vec![out.stdout, out.stderr]);
    }
    Ok(PathBuf::from(out.stdout.trim()))
}

pub(crate) fn doctor(ctx: &Context, root: &Path) -> Outcome {
    crate::doctor::inventory(ctx, root, &["--gpu", "--sim"])
}

pub(crate) fn arm(ctx: &Context, root: &Path) -> Outcome {
    let mut lines = command(
        ctx,
        root,
        "python3",
        &[
            "-I",
            "tools/output.py",
            "build",
            "--release",
            "--target",
            crate::repo::TARGET,
            "-p",
            "oos-ui",
            "-p",
            "oos-fpgad",
            "--bins",
        ],
    )?;
    let target = target(ctx, root)?.join(crate::repo::TARGET).join("release");
    for name in ["oos-ui", "oos-fpgad", "oos-update"] {
        let path = target.join(name);
        lines.extend(command(ctx, root, "python3", &["-I", "tools/output.py", "verify", &path.to_string_lossy()])?);
        crate::elf::file(&path).map_err(|e| vec![e])?;
        lines.push(format!("arm64-release-link: {name}: receipt and static AArch64 ELF verified"));
    }
    Ok(lines)
}

fn simulation(ctx: &Context, root: &Path, builders: &[&str], script: &str) -> Outcome {
    let mut commands = vec![vec!["tools/build-env.sh".to_string(), "prepare".into(), "initramfs".into()]];
    for builder in builders {
        commands.push(vec!["tools/build-env.sh".into(), "prepare".into(), format!("rootfs-{builder}")]);
    }
    commands.extend(builders.iter().map(|builder| vec![format!("rootfs/build-{builder}.sh")]));
    commands.push(vec![script.to_string()]);
    let mut lines = Vec::new();
    for args in commands {
        let args: Vec<_> = ["-u", "OOS_OUT", "sh"].into_iter().chain(args.iter().map(String::as_str)).collect();
        match command(ctx, root, "env", &args) {
            Ok(result) => lines.extend(result),
            Err(result) => {
                lines.extend(result);
                return Err(lines);
            }
        }
    }
    Ok(lines)
}

pub(crate) fn ssh(ctx: &Context, root: &Path) -> Outcome {
    simulation(ctx, root, &["nft", "dropbear", "sftp-server"], "tools/ssh-sim.sh")
}
pub(crate) fn firewall(ctx: &Context, root: &Path) -> Outcome {
    simulation(ctx, root, &["nft"], "tools/fw-sim.sh")
}
pub(crate) fn gpu(ctx: &Context, root: &Path) -> Outcome {
    command(ctx, root, "sh", &["gpu/test-host.sh"])
}
