// SPDX-License-Identifier: GPL-2.0-or-later
//! The tracked files of the working tree, and a git ref extracted to a temporary directory to be measured there.

use crate::run;
use std::path::{Path, PathBuf};

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = run::capture(&Default::default(), root, "git", args)?;
    if !out.ok {
        return Err(format!("git {}: {}", args.first().unwrap_or(&""), run::tail(&out.stderr, 5)));
    }
    Ok(out.stdout)
}

/// The tracked paths of the working tree with its new files (`rev` None), or those of `rev`.
pub fn files(root: &Path, rev: Option<&str>) -> Result<Vec<String>, String> {
    let args: &[&str] = match rev {
        None => &["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        Some(r) => &["ls-tree", "-r", "-z", "--name-only", r],
    };
    Ok(git(root, args)?.split('\0').filter(|s| !s.is_empty()).map(String::from).collect())
}

/// The files a count reads: Rust sources, manifests, the lock file and scripts. Not the toolchain pin: cargo in the
/// extracted tree runs with the current toolchain instead of installing an old one.
pub fn needed(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let kept = [".rs", ".toml", ".lock", ".sh", ".py"];
    name != "rust-toolchain.toml" && (!name.contains('.') || kept.iter().any(|e| name.ends_with(e)))
}

fn extract(root: &Path, rev: &str, files: &[String], dir: &Path, tar: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {}", dir.display(), e))?;
    let out = tar.to_string_lossy().into_owned();
    let mut args = vec!["--literal-pathspecs", "archive", "--format=tar", "-o", &out, rev, "--"];
    args.extend(files.iter().map(String::as_str));
    git(root, &args)?;
    let x = run::capture(&Default::default(), dir, "tar", &["-xf", &out])?;
    if !x.ok {
        return Err(format!("tar: {}", run::tail(&x.stderr, 5)));
    }
    Ok(())
}

/// Runs `measure` on `rev` extracted to a temporary directory, which is removed afterwards.
pub fn at<T>(root: &Path, rev: &str, measure: impl Fn(&Path, &[String]) -> Result<T, String>) -> Result<T, String> {
    let files: Vec<String> = files(root, Some(rev))?.into_iter().filter(|f| needed(f)).collect();
    let stamp =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let dir = std::env::temp_dir().join(format!("xtask-loc-{}-{}", std::process::id(), stamp));
    let tar: PathBuf = dir.with_extension("tar");
    let result = extract(root, rev, &files, &dir, &tar).and_then(|()| measure(&dir, &files));
    let cleaned = [dir.as_path(), tar.as_path()].into_iter().filter(|p| p.exists()).try_for_each(|p| {
        let r = if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) };
        r.map_err(|e| format!("{}: {}", p.display(), e))
    });
    let r = result.map_err(|e| format!("{}: {}", rev, e))?;
    cleaned.map(|()| r)
}
