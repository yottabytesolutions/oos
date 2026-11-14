use super::*;

fn r(t: &str) -> Ratchet {
    Ratchet::parse(t).expect("ratchet")
}

// The hole: an edit of the ratchet file authorized a new or a grown finding.
#[test]
fn a_ratchet_file_may_only_shrink_against_the_base() {
    let base = r("5 fn a.rs f\n3 file a.rs\n");
    assert!(widened(&base, &r("4 fn a.rs f\n")).is_empty());
    assert_eq!(widened(&base, &r("6 fn a.rs f\n3 file a.rs\n")), ["fn a.rs f: raised in the ratchet file from 5 to 6"]);
    assert_eq!(widened(&base, &r("1 fn b.rs g\n")), ["fn b.rs g: added to the ratchet file (fix the finding instead)"]);
}

#[test]
fn coverage_floors_may_only_rise_against_the_base() {
    let base = r("900 ui\n800 hal\n");
    assert!(lowered(&base, &r("910 ui\n800 hal\n700 core\n")).is_empty());
    assert_eq!(
        lowered(&base, &r("899 ui\n")),
        ["hal: coverage floor removed (base 800)", "ui: coverage floor lowered from 900 to 899"]
    );
}

// The hole: OOS_GATE_BASE=HEAD compared the committed ratchet with itself.
#[test]
fn the_base_override_counts_only_in_ci() {
    let all = |_: &str| true;
    assert_eq!(select(false, Some("HEAD"), all).expect("base"), "overhaul");
    assert_eq!(select(true, Some("main"), all).expect("base"), "main");
    assert_eq!(select(false, None, |r: &str| r == "origin/overhaul").expect("base"), "origin/overhaul");
    assert!(select(false, None, |_: &str| false).is_err());
}

/// A git repository in a fresh temporary directory, with `steps` of (files to write, commit message).
fn repo(name: &str, steps: &[(&[(&str, &str)], &str)]) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("xtask-base-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("dir");
    let g = |a: &[&str]| assert!(run::capture(&d, "git", a).expect("git").ok, "git {:?}", a);
    g(&["init", "-q", "-b", "overhaul"]);
    for (files, msg) in steps {
        for (p, t) in *files {
            let f = d.join(p);
            std::fs::create_dir_all(f.parent().expect("parent")).expect("dir");
            std::fs::write(f, t).expect("write");
        }
        g(&["add", "-A"]);
        g(&["-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", msg]);
    }
    d
}

// The holes: on the base branch the merge base was HEAD itself; a missing file at a base that already had the gate
// was taken as "no ratchet yet".
#[test]
fn the_base_is_the_parent_on_the_branch_and_a_lost_file_fails() {
    let d = repo(
        "branch",
        &[
            (&[("README", "x")], "before the gate"),
            (&[("xtask/Cargo.toml", "x"), ("xtask/data/size.txt", "5 fn a.rs f\n")], "gate"),
            (&[("xtask/data/size.txt", "9 fn a.rs f\n")], "raise"),
        ],
    );
    let c = commit(&d, "overhaul").expect("commit");
    assert_eq!(text_at(&d, &c, "xtask/data/size.txt").expect("text").as_deref(), Some("5 fn a.rs f"));
    assert!(text_at(&d, &c, "xtask/data/unsafe.txt").is_err());
    let first = git(&d, &["rev-list", "--max-parents=0", "HEAD"]).expect("root");
    assert_eq!(text_at(&d, &first, "xtask/data/size.txt").expect("text"), None);
    assert!(text_at(&d, "no-such-commit", "xtask/data/size.txt").is_err());
    std::fs::remove_dir_all(&d).expect("clean up");
}
