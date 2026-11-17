// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::fs;

fn temp(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("clock").as_nanos();
    let dir = std::env::temp_dir().join(format!("oos-git-env-{}-{}-{stamp}", name, std::process::id()));
    fs::create_dir_all(&dir).expect("temp dir");
    dir
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(dir).env_remove("GIT_DIR").output().expect("git");
    assert!(out.status.success(), "git {:?}: {}", args, String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn a_hook_repository_variable_never_reaches_a_fixture_command() {
    let sentinel = temp("sentinel");
    git(&sentinel, &["init", "-q", "-b", "main"]);
    let config_before = fs::read_to_string(sentinel.join(".git/config")).expect("sentinel config");
    let fixture = temp("fixture");
    // What a hook exports: the variables point at the sentinel, as they would at the real clone.
    let mut command = Command::new("git");
    command
        .env("GIT_DIR", sentinel.join(".git"))
        .env("GIT_WORK_TREE", &sentinel)
        .env("GIT_INDEX_FILE", sentinel.join(".git/index"));
    let out = execute(&Context::default(), &fixture, "git", &["init", "-q", "-b", "fixture"], &[], &mut command)
        .expect("git starts");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert!(fixture.join(".git").is_dir(), "the fixture got its own repository");
    assert_eq!(fs::read_to_string(sentinel.join(".git/config")).expect("sentinel config"), config_before);
    assert_eq!(git(&sentinel, &["symbolic-ref", "HEAD"]).trim(), "refs/heads/main");
    for dir in [sentinel, fixture] {
        fs::remove_dir_all(dir).expect("clean up");
    }
}

#[test]
fn explicit_overrides_still_apply() {
    let dir = temp("override");
    let out = capture_env(
        &Context::default(),
        &dir,
        "sh",
        &["-c", "printf %s \"$GIT_DIR\""],
        &[("GIT_DIR", std::ffi::OsStr::new("x"))],
    )
    .expect("sh starts");
    assert_eq!(out.stdout, "x");
    fs::remove_dir_all(dir).expect("clean up");
}
