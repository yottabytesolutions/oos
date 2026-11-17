use super::*;
use crate::testrepo::{commit, git, repo};

const SIZE: &str = "xtask/data/ui/size.txt";

/// A base on `main` with the gate, our file `ui/src/ours.rs` listed at 400; branch `integ` with our own change,
/// then upstream's history (its own root, bridged with strategy ours), then a merge of upstream's change to
/// `ui/src/up.rs`. Returns the repository and the upstream commit.
fn integration(name: &str) -> (std::path::PathBuf, String) {
    let d = repo(
        name,
        &[(&[("xtask/Cargo.toml", "x"), (SIZE, "400 file ui/src/ours.rs\n"), ("ui/src/ours.rs", "a")], "base")],
    );
    git(&d, &["checkout", "-q", "-b", "integ"]);
    commit(&d, &[("ui/src/ours.rs", "b")], "our work");
    (d.clone(), merge_upstream(&d))
}

/// On `integ`: our copy of `ui/src/up.rs`, an upstream root, a bridge to it, an upstream change to `ui/src/up.rs` and its merge.
fn merge_upstream(d: &std::path::Path) -> String {
    let m = |args: &[&str]| {
        let mut a = vec!["-c", "user.name=t", "-c", "user.email=t@t", "merge", "-q", "--no-ff"];
        a.extend(args);
        git(d, &a);
    };
    // our tree already holds upstream's file as it was (upstream's copy of our work)
    commit(d, &[("ui/src/up.rs", "a")], "our copy");
    git(d, &["checkout", "-q", "--orphan", "up"]);
    git(d, &["rm", "-rq", "--cached", "."]);
    git(d, &["clean", "-fdq"]);
    commit(d, &[("ui/src/up.rs", "a")], "upstream root");
    git(d, &["checkout", "-q", "integ"]);
    m(&["--allow-unrelated-histories", "-s", "ours", "-m", "bridge", "up"]);
    git(d, &["checkout", "-q", "up"]);
    commit(d, &[("ui/src/up.rs", "upstream")], "upstream work");
    let up = head(d);
    git(d, &["checkout", "-q", "integ"]);
    m(&["-m", "merge upstream", "up"]);
    up
}

fn head(d: &std::path::Path) -> String {
    let out = run::capture(&Default::default(), d, "git", &["rev-parse", "HEAD"]).expect("git");
    out.stdout.trim().to_string()
}

fn rebaseline(d: &std::path::Path, files: &[(&str, &str)], trailer: Option<&str>) {
    let msg = match trailer {
        Some(sha) => format!("Rebaseline ratchets\n\n{}: upstream {}\n", TRAILER, sha),
        None => "Rebaseline ratchets\n".to_string(),
    };
    commit(d, files, &msg);
}

fn base_of(d: &std::path::Path) -> String {
    crate::base::protected(&Default::default(), d).expect("base")
}

#[test]
fn a_rebaseline_admits_upstream_entries() {
    let (d, up) = integration("rebaseline-ok");
    rebaseline(&d, &[(SIZE, "400 file ui/src/ours.rs\n520 file ui/src/up.rs\n")], Some(&up));
    let a = admitted(&Default::default(), &d, &base_of(&d)).expect("admitted");
    assert_eq!(a[SIZE].0.get("file ui/src/up.rs"), Some(&520));
    assert_eq!(a[SIZE].0.len(), 1);
    let mut base = Ratchet::parse("400 file ui/src/ours.rs\n").expect("r");
    widen(&mut base, &a[SIZE]);
    assert!(
        crate::base::widened(&base, &crate::store::load(&d, "ui", crate::store::Kind::Size).expect("load")).is_empty()
    );
}

#[test]
fn a_rebaseline_of_a_file_upstream_did_not_change_fails() {
    let (d, up) = integration("rebaseline-other");
    rebaseline(&d, &[(SIZE, "400 file ui/src/ours.rs\n450 file ui/src/new.rs\n")], Some(&up));
    let e = admitted(&Default::default(), &d, &base_of(&d)).expect_err("rejected");
    assert!(e.contains("ui/src/new.rs"), "{}", e);
}

#[test]
fn a_commit_without_the_trailer_admits_nothing() {
    let (d, _) = integration("rebaseline-no-trailer");
    rebaseline(&d, &[(SIZE, "400 file ui/src/ours.rs\n520 file ui/src/up.rs\n")], None);
    assert!(admitted(&Default::default(), &d, &base_of(&d)).expect("admitted").is_empty());
}

#[test]
fn a_rebaseline_that_touches_code_fails() {
    let (d, up) = integration("rebaseline-code");
    rebaseline(&d, &[(SIZE, "400 file ui/src/ours.rs\n520 file ui/src/up.rs\n"), ("ui/src/up.rs", "x")], Some(&up));
    let e = admitted(&Default::default(), &d, &base_of(&d)).expect_err("rejected");
    assert!(e.contains("only xtask/data"), "{}", e);
}

#[test]
fn a_rebaseline_that_raises_our_own_file_fails() {
    let (d, up) = integration("rebaseline-ours");
    rebaseline(&d, &[(SIZE, "450 file ui/src/ours.rs\n")], Some(&up));
    let e = admitted(&Default::default(), &d, &base_of(&d)).expect_err("rejected");
    assert!(e.contains("ui/src/ours.rs"), "{}", e);
}

#[test]
fn a_rebaseline_without_an_upstream_merge_fails() {
    let d = repo("rebaseline-no-merge", &[(&[("xtask/Cargo.toml", "x"), (SIZE, "")], "base")]);
    git(&d, &["checkout", "-q", "-b", "integ"]);
    commit(&d, &[("ui/src/up.rs", "ours")], "our work");
    let ours = head(&d);
    rebaseline(&d, &[(SIZE, "520 file ui/src/up.rs\n")], Some(&ours));
    let e = admitted(&Default::default(), &d, &base_of(&d)).expect_err("rejected");
    assert!(e.contains("no merge of upstream"), "{}", e);
}

#[test]
fn crate_keys_need_an_impl_upstream_added_in_the_crate() {
    let changed =
        Changed { files: ["ui/src/up.rs".to_string()].into(), impls: ["ui Ui".to_string()].into(), moved: Vec::new() };
    assert!(changed.covers("impls ui Ui"));
    assert!(!changed.covers("impls ui Other"));
    assert!(!changed.covers("impls hal Ui"));
    assert!(!changed.covers("lines ui Ui"));
    assert!(changed.covers("ui ui/src/up.rs clippy::x"));
    assert!(!changed.covers("ui ui/src/ours.rs clippy::x"));
}

#[test]
fn impl_headers_name_their_type() {
    assert_eq!(impl_type("impl Ui {"), Some("Ui"));
    assert_eq!(impl_type("    impl<'a> Foo<'a> {"), Some("Foo"));
    assert_eq!(impl_type("impl crate::ui::Ui {"), Some("Ui"));
    assert_eq!(impl_type("impl std::fmt::Display for Ui {"), None);
    assert_eq!(impl_type("implement(x)"), None);
    let diff = "+++ b/ui/src/ui/new.rs\n+impl Ui {\n+    fn f() {}\n+++ b/hal/src/a.rs\n+impl Dev {\n";
    assert_eq!(added_impls(diff), ["hal Dev".to_string(), "ui Ui".to_string()].into());
}

/// The base of `integration` with a moves file that carried `Model` out of the upstream file into core.
fn integration_with_move(name: &str) -> (std::path::PathBuf, String) {
    let moves = "struct ui/src/up.rs Model -> struct core/src/model.rs Model\n";
    let d = repo(name, &[(&[("xtask/Cargo.toml", "x"), (SIZE, ""), ("xtask/data/moves.txt", moves)], "base")]);
    git(&d, &["checkout", "-q", "-b", "integ"]);
    (d.clone(), merge_upstream(&d))
}

#[test]
fn a_destination_of_a_recorded_move_is_admitted() {
    let (d, up) = integration_with_move("rebaseline-moved");
    commit(&d, &[("xtask/data/core/size.txt", "195 struct core/src/model.rs Model\n")], "port");
    rebaseline(&d, &[("xtask/data/core/size.txt", "196 struct core/src/model.rs Model\n")], Some(&up));
    let a = admitted(&Default::default(), &d, &base_of(&d)).expect("admitted");
    assert_eq!(a["xtask/data/core/size.txt"].0.get("struct core/src/model.rs Model"), Some(&196));
}

#[test]
fn an_unlisted_destination_fails() {
    let (d, up) = integration_with_move("rebaseline-unlisted");
    rebaseline(&d, &[("xtask/data/core/size.txt", "80 fn core/src/model.rs Model::new\n")], Some(&up));
    let e = admitted(&Default::default(), &d, &base_of(&d)).expect_err("rejected");
    assert!(e.contains("Model::new"), "{}", e);
}

#[test]
fn the_fast_gate_holds_upstream_findings_only_until_the_rebaseline() {
    let (d, up) = integration("rebaseline-pending");
    let p = pending(&Default::default(), &d, &base_of(&d)).expect("pending");
    assert!(p.covers("fn ui/src/up.rs f") && !p.covers("fn ui/src/ours.rs f"));
    rebaseline(&d, &[(SIZE, "400 file ui/src/ours.rs\n70 fn ui/src/up.rs f\n")], Some(&up));
    assert!(pending(&Default::default(), &d, &base_of(&d)).expect("pending").files.is_empty());
}

#[test]
fn a_task_branch_merge_is_not_upstream() {
    let d = repo("rebaseline-task", &[(&[("xtask/Cargo.toml", "x"), (SIZE, "")], "base")]);
    git(&d, &["checkout", "-q", "-b", "t0001"]);
    commit(&d, &[("ui/src/task.rs", "ours")], "task work");
    git(&d, &["checkout", "-q", "-b", "integ", "main"]);
    git(&d, &["-c", "user.name=t", "-c", "user.email=t@t", "merge", "-q", "--no-ff", "-m", "merge task", "t0001"]);
    assert!(pending(&Default::default(), &d, &base_of(&d)).expect("pending").files.is_empty());
}

#[test]
fn a_new_file_move_of_an_upstream_file_is_admitted_but_not_onto_our_code() {
    let (d, up) = integration("rebaseline-file-move");
    let moved = "ui/src/up.rs -> ui/src/up/inner.rs\nui/src/up.rs -> ui/src/ours.rs\n";
    commit(&d, &[("xtask/data/moves.txt", moved)], "relocate the upstream module");
    let m = moves(&Default::default(), &d, &base_of(&d), Some("HEAD")).expect("moves");
    assert!(m.iter().any(|m| m.new == "ui/src/up/inner.rs"));
    assert!(!m.iter().any(|m| m.new == "ui/src/ours.rs"), "an existing file of ours is no destination");
    rebaseline(&d, &[(SIZE, "400 file ui/src/ours.rs\n70 fn ui/src/up/inner.rs f\n")], Some(&up));
    let a = admitted(&Default::default(), &d, &base_of(&d)).expect("admitted");
    assert_eq!(a[SIZE].0.get("fn ui/src/up/inner.rs f"), Some(&70));
}

#[test]
fn a_carried_key_may_move_on_but_an_unrelated_key_move_carries_nothing() {
    let (d, up) = integration_with_move("rebaseline-key-chain");
    let moved = "struct core/src/model.rs Model -> struct core/src/model/state.rs Model\n\
                 struct ui/src/ours.rs Big -> struct ui/src/big.rs Big\n";
    commit(&d, &[("xtask/data/moves.txt", moved)], "move the model on");
    let m = moves(&Default::default(), &d, &base_of(&d), Some("HEAD")).expect("moves");
    let changed = Changed { files: ["ui/src/up.rs".to_string()].into(), impls: BTreeSet::new(), moved: m };
    assert!(changed.covers("struct core/src/model/state.rs Model"));
    assert!(!changed.covers("struct ui/src/big.rs Big"));
    let _ = up;
}

/// A second intake: the base already holds an upstream merge, so the next upstream commit's merge base with ours is
/// in the base (on the second-parent line). Before, that merge did not count as upstream and nothing was pending.
#[test]
fn a_second_upstream_merge_after_one_in_the_base_is_upstream() {
    let (d, _) = integration("rebaseline-second");
    git(&d, &["checkout", "-q", "main"]);
    git(&d, &["-c", "user.name=t", "-c", "user.email=t@t", "merge", "-q", "--ff-only", "integ"]);
    git(&d, &["checkout", "-q", "-b", "integ2"]);
    git(&d, &["checkout", "-q", "up"]);
    commit(&d, &[("ui/src/up2.rs", "more upstream")], "upstream work 2");
    git(&d, &["checkout", "-q", "integ2"]);
    git(&d, &["-c", "user.name=t", "-c", "user.email=t@t", "merge", "-q", "--no-ff", "-m", "merge upstream 2", "up"]);
    let p = pending(&Default::default(), &d, &base_of(&d)).expect("pending");
    assert!(p.covers("fn ui/src/up2.rs f") && !p.covers("fn ui/src/ours.rs f") && !p.covers("fn ui/src/up.rs f"));
}
