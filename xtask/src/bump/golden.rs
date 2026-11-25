// SPDX-License-Identifier: GPL-2.0-or-later

use super::{read, Edit, Error, Version};
use crate::run::{self, Context};
use std::fs;
use std::path::{Component, Path, PathBuf};

const REFERENCES: &str = "ui/tests/golden";
const RENDERER: &str = "ui/tools/golden_version.py";

pub(super) fn check(ctx: &Context, root: &Path, version: Version) -> Result<(), Vec<String>> {
    let out = run::capture(ctx, root, "python3", &["-B", "-E", RENDERER, &version.to_string()])
        .map_err(|error| vec![error])?;
    if out.ok {
        Ok(())
    } else if out.stderr.is_empty() {
        Err(vec!["release golden check failed without diagnostics".into()])
    } else {
        Err(out.stderr.lines().map(str::to_owned).collect())
    }
}

pub(super) fn stage(root: &Path, version: Version) -> Result<Vec<Edit>, Error> {
    let target = command(root, "python3", &["-I", "tools/output.py", "target"], &[])?;
    let target = Path::new(target.trim());
    let scratch = Scratch::create(target.join(format!("release-bump-{}", std::process::id())))?;
    let inventory = scratch.0.join("inventory");
    command(
        root,
        "cargo",
        &[
            "test",
            "--offline",
            "--locked",
            "--profile",
            "test-release",
            "-p",
            "oos-ui",
            "--test",
            "golden",
            "export_update_inventory",
            "--",
            "--ignored",
            "--exact",
        ],
        &[("OOS_GOLDEN_INVENTORY", inventory.as_os_str()), ("CARGO_TARGET_DIR", target.as_os_str())],
    )?;
    let stdout = command(
        root,
        "python3",
        &[
            "-B",
            "-E",
            RENDERER,
            &version.to_string(),
            "--stage",
            path_text(&scratch.0)?,
            "--inventory",
            path_text(&inventory.join("inventory.tsv"))?,
            "--binary",
            path_text(&target.join("test-release/oos-ui"))?,
        ],
        &[],
    )?;
    stdout.lines().chain(["manifest.txt"]).map(|name| staged_edit(root, &scratch.0, name)).collect()
}

fn staged_edit(root: &Path, scratch: &Path, name: &str) -> Result<Edit, Error> {
    let path = Path::new(name);
    if path.components().any(|part| !matches!(part, Component::Normal(_)))
        || (name != "manifest.txt" && path.extension().is_none_or(|ext| ext != "png"))
    {
        return Err(Error::Format(format!("invalid staged golden path: {name}")));
    }
    let path = Path::new(REFERENCES).join(path);
    let before = read(root, &path)?;
    let after = read(scratch, &Path::new("references").join(name))?;
    Ok(Edit::new(path, before, after, Vec::new()))
}

fn path_text(path: &Path) -> Result<&str, Error> {
    path.to_str().ok_or_else(|| Error::Format("golden build paths must be UTF-8".into()))
}

fn command(root: &Path, program: &str, args: &[&str], env: &[(&str, &std::ffi::OsStr)]) -> Result<String, Error> {
    let out = run::capture_env(&Context::default(), root, program, args, env).map_err(Error::Command)?;
    if out.ok {
        Ok(out.stdout)
    } else {
        Err(Error::Command(format!("{program} {} failed:\n{}{}", args.join(" "), out.stdout, out.stderr)))
    }
}

#[derive(Debug)]
pub(super) struct Scratch(PathBuf);

impl Scratch {
    pub(super) fn create(path: PathBuf) -> Result<Self, Error> {
        let parent = path.parent().ok_or_else(|| Error::Format("missing scratch parent".into()))?;
        fs::create_dir_all(parent)
            .and_then(|()| fs::create_dir(&path))
            .map_err(|error| Error::File(path.clone(), error))?;
        Ok(Self(path))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("{}: remove release scratch directory: {error}", self.0.display());
        }
    }
}
