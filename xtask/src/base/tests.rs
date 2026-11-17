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
    assert_eq!(select(false, Some("HEAD"), all).expect("base"), "main");
    assert_eq!(select(true, Some("release"), all).expect("base"), "release");
    assert_eq!(select(false, None, |r: &str| r == "origin/main").expect("base"), "origin/main");
    assert!(select(false, None, |_: &str| false).is_err());
}

use crate::testrepo::repo;

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
    let c = commit(&Default::default(), &d, "main").expect("commit");
    assert_eq!(
        text_at(&Default::default(), &d, &c, "xtask/data/size.txt").expect("text").as_deref(),
        Some("5 fn a.rs f")
    );
    assert!(text_at(&Default::default(), &d, &c, "xtask/data/unsafe.txt").is_err());
    let first = git(&Default::default(), &d, &["rev-list", "--max-parents=0", "HEAD"]).expect("root");
    assert_eq!(text_at(&Default::default(), &d, &first, "xtask/data/size.txt").expect("text"), None);
    assert!(text_at(&Default::default(), &d, "no-such-commit", "xtask/data/size.txt").is_err());
    std::fs::remove_dir_all(&d).expect("clean up");
}

// The split: a base with one combined file per kind gives each crate its own entries; a crate or kind the base does
// not know starts empty; a base without the gate has nothing to compare.
#[test]
fn per_crate_ratchets_read_a_combined_base() {
    use crate::store::{at_base, Kind};
    let d = repo(
        "split",
        &[
            (&[("README", "x")], "before the gate"),
            (
                &[("xtask/Cargo.toml", "x"), ("xtask/data/size.txt", "5 fn ui/src/a.rs f\n7 fn hal/src/b.rs g\n")],
                "gate",
            ),
            (&[("xtask/data/ui/size.txt", "5 fn ui/src/a.rs f\n")], "split"),
        ],
    );
    let head = git(&Default::default(), &d, &["rev-parse", "HEAD"]).expect("head");
    let gate = git(&Default::default(), &d, &["rev-parse", "HEAD^1"]).expect("gate");
    let first = git(&Default::default(), &d, &["rev-list", "--max-parents=0", "HEAD"]).expect("root");
    let r = |t: &str| Some(Ratchet::parse(t).expect("r"));
    assert_eq!(at_base(&Default::default(), &d, &gate, "hal", Kind::Size).expect("at"), r("7 fn hal/src/b.rs g\n"));
    assert_eq!(at_base(&Default::default(), &d, &head, "ui", Kind::Size).expect("at"), r("5 fn ui/src/a.rs f\n"));
    assert_eq!(at_base(&Default::default(), &d, &head, "core", Kind::Size).expect("at"), r(""));
    assert_eq!(at_base(&Default::default(), &d, &gate, "ui", Kind::Known).expect("at"), r(""));
    assert_eq!(at_base(&Default::default(), &d, &first, "ui", Kind::Size).expect("at"), None);
    std::fs::remove_dir_all(&d).expect("clean up");
}
