//! Test fixture: a git repository in a fresh temporary directory, with `main` as its first branch.

use crate::run;
use std::path::{Path, PathBuf};

/// Runs git in `dir`; panics on failure (test code).
pub fn git(dir: &Path, args: &[&str]) {
    let env =
        [("GIT_CONFIG_GLOBAL", std::ffi::OsStr::new("/dev/null")), ("GIT_CONFIG_NOSYSTEM", std::ffi::OsStr::new("1"))];
    let out = run::capture_env(&Default::default(), dir, "git", args, &env).expect("git");
    assert!(out.ok, "git {:?}: {}", args, out.stderr);
}

/// Writes `files` and commits them with `msg`.
pub fn commit(dir: &Path, files: &[(&str, &str)], msg: &str) {
    for (p, t) in files {
        let f = dir.join(p);
        std::fs::create_dir_all(f.parent().expect("parent")).expect("dir");
        std::fs::write(f, t).expect("write");
    }
    git(dir, &["add", "-A"]);
    git(dir, &["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", msg]);
}

/// A repository named `name` on branch `main`, with `steps` of (files to write, commit message).
pub fn repo(name: &str, steps: &[(&[(&str, &str)], &str)]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("xtask-repo-{}-{}", name, std::process::id()));
    if d.exists() {
        std::fs::remove_dir_all(&d).expect("clean up");
    }
    std::fs::create_dir_all(&d).expect("dir");
    git(&d, &["init", "-q", "-b", "main"]);
    // Disposable synthetic histories must not depend on the host signing agent.
    git(&d, &["config", "commit.gpgsign", "false"]);
    for (files, msg) in steps {
        commit(&d, files, msg);
    }
    d
}
