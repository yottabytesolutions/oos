// SPDX-License-Identifier: GPL-2.0-or-later
//! The comparison every ratchet step shares: the current entries of each crate against its ratchet file, after the
//! planned moves, with the file itself checked against the base.

use super::{outcome, Outcome};
use crate::base::{self, At};
use crate::moves::{self, Move};
use crate::ratchet::Ratchet;
use crate::store::{self, Kind};
use std::collections::BTreeSet;
use std::path::Path;

/// The planned moves at the base and on the branch. `ratchet --move` keeps the moves the base still needs, so they
/// explain the renamed entries in the commit that applies them; at a base that has them they change nothing.
fn all_moves(ctx: &crate::run::Context, root: &Path, commit: Option<&str>) -> Result<Vec<Move>, String> {
    let mut m = match commit.map(|c| base::at(ctx, root, c, "xtask/data/moves.txt")).transpose()? {
        Some(At::Present(t)) => moves::parse(&t)?,
        _ => Vec::new(),
    };
    m.extend(load_moves(root)?);
    Ok(m)
}

/// The moves file of the working tree (missing: none).
pub fn load_moves(root: &Path) -> Result<Vec<Move>, String> {
    match std::fs::read_to_string(root.join("xtask/data/moves.txt")) {
        Ok(t) => moves::parse(&t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("xtask/data/moves.txt: {}", e)),
    }
}

/// The allowed entries of every ratcheted crate for `kind`, moves applied; their base, moves applied, when there is
/// one.
fn allowed(
    ctx: &crate::run::Context,
    root: &Path,
    kind: Kind,
    crates: &[&str],
    commit: &str,
) -> Result<(Ratchet, Option<Ratchet>), String> {
    let moves = all_moves(ctx, root, Some(commit))?;
    let (mut work, mut base) = (Ratchet::default(), Some(Ratchet::default()));
    for k in crates {
        work.0.extend(store::load(root, k, kind)?.0);
        base = match (base, store::at_base(ctx, root, commit, k, kind)?) {
            (Some(mut b), Some(at)) => {
                b.0.extend(at.0);
                Some(b)
            }
            _ => None,
        };
    }
    let base = base.map(|b| moves::apply(&b, &moves)).transpose()?;
    Ok((moves::apply(&work, &moves)?, base))
}

/// An impl removed from a changed file still makes that file a constituent of the type's spread at the base.
fn impl_touched(
    ctx: &crate::run::Context,
    root: &Path,
    commit: &str,
    krate: &str,
    ty: &str,
    changed: &BTreeSet<String>,
) -> Result<bool, String> {
    let prefix = format!("{krate}/");
    for file in changed.iter().filter(|f| f.starts_with(&prefix) && f.ends_with(".rs")) {
        let current = match std::fs::read_to_string(root.join(file)) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(format!("{file}: {e}")),
        };
        let before = match base::at(ctx, root, commit, file)? {
            At::Present(text) => Some(text),
            _ => None,
        };
        for text in [before, current].into_iter().flatten() {
            let impls = crate::size::inherent_impls(&text).map_err(|e| format!("{file}: {e}"))?;
            if impls.iter().any(|name| name == ty) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// Compares `now` of every crate (`(crate, entries)`) with the ratchet files of `kind`, moves applied. A crate allows
/// what its files list and what the moves bring into it; a new crate without files starts clean.
pub fn ratcheted(ctx: &crate::run::Context, root: &Path, kind: Kind, now: &[(&'static str, Ratchet)]) -> Outcome {
    let crates: Vec<&str> = now.iter().map(|n| n.0).collect();
    let commit = base::protected(ctx, root).map_err(|e| vec![e])?;
    let (work, base) = allowed(ctx, root, kind, &crates, &commit).map_err(|e| vec![e])?;
    if let Some(b) = &base {
        let widened = base::widened(b, &work);
        if !widened.is_empty() {
            return Err(widened.into_iter().map(|w| format!("{} ratchet: {}", kind.name(), w)).collect());
        }
    }
    let changed = base::changed_files(ctx, root, &commit).map_err(|e| vec![e])?;
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for (krate, n) in now {
        let v = store::of_crate(&work, krate).compare(n);
        bad.extend(v.failures().into_iter().map(|f| format!("{}: {}", krate, f)));
        for (key, allowed, now) in &v.shrunk {
            let touched = match key.strip_prefix(&format!("impls {krate} ")).filter(|_| kind == Kind::Size) {
                Some(ty) => impl_touched(ctx, root, &commit, krate, ty, &changed).map_err(|e| vec![e])?,
                None => key.split(' ').nth(1).is_some_and(|file| changed.contains(file)),
            };
            if touched {
                bad.push(format!(
                    "{krate}: {key} has unused ratchet slack: {allowed} -> {now}; run cargo xtask ratchet"
                ));
            }
        }
        let total: u64 = n.0.values().sum();
        let shrunk = if v.shrunk.is_empty() {
            String::new()
        } else {
            format!(", {} improved (cargo xtask ratchet)", v.shrunk.len())
        };
        ok.push(format!("{}: {} entries, {} total{}", krate, n.0.len(), total, shrunk));
    }
    outcome(ok, bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testrepo::{commit, git, repo};

    fn r(t: &str) -> Ratchet {
        Ratchet::parse(t).expect("r")
    }

    #[test]
    fn impl_spread_slack_requires_a_changed_constituent_file() {
        for (name, allowance, current, delete) in
            [("reduced", 5, 4, false), ("fixed", 4, 0, false), ("deleted", 5, 4, true)]
        {
            let d = repo(
                &format!("impl-slack-{name}"),
                &[(
                    &[
                        ("xtask/Cargo.toml", "x"),
                        ("xtask/data/ui/size.txt", &format!("{allowance} impls ui Ui\n")),
                        ("ui/src/a.rs", "impl Ui {}\n"),
                        ("ui/src/b.rs", "impl Other {}\n"),
                        ("core/src/a.rs", "impl Ui {}\n"),
                        ("ui/src/tests.rs", "#[cfg(test)] mod tests { impl Ui {} }\n"),
                        ("ui/src/traits.rs", "impl Clone for Ui {}\n"),
                    ],
                    "base",
                )],
            );
            git(&d, &["checkout", "-q", "-b", "change"]);
            commit(&d, &[("README", "unrelated change")], "change");
            let now = if current == 0 { Ratchet::default() } else { r(&format!("{current} impls ui Ui\n")) };
            let check = || ratcheted(&Default::default(), &d, Kind::Size, &[("ui", now.clone())]);
            assert!(check().is_ok(), "untouched slack must pass");
            for file in ["ui/src/b.rs", "core/src/a.rs", "ui/src/tests.rs", "ui/src/traits.rs"] {
                let path = d.join(file);
                let text = std::fs::read_to_string(&path).expect("read");
                std::fs::write(path, format!("{text}// unrelated edit\n")).expect("edit");
            }
            assert!(check().is_ok(), "unrelated impls must not require tightening");
            if delete {
                std::fs::remove_file(d.join("ui/src/a.rs")).expect("delete");
            } else {
                std::fs::write(d.join("ui/src/a.rs"), "").expect("remove impl");
            }
            assert_eq!(
                check().expect_err("impl slack"),
                [format!(
                    "ui: impls ui Ui has unused ratchet slack: {allowance} -> {current}; run cargo xtask ratchet"
                )]
            );
            store::save(&d, "ui", Kind::Size, &now).expect("tighten");
            assert!(check().is_ok(), "tightened impl spread must pass");
            std::fs::remove_dir_all(&d).expect("clean up");
        }
    }

    #[test]
    fn unused_slack_requires_tightening_only_for_touched_files() {
        for (kind, prefix) in [
            (Kind::Size, "fn"),
            (Kind::Clippy, "ui"),
            (Kind::Statics, "static"),
            (Kind::Unsafe, "unsafe"),
            (Kind::Allow, "allow"),
            (Kind::Known, "known"),
            (Kind::TestTime, "time"),
        ] {
            let a = format!("{prefix} ui/src/a.rs finding");
            let b = format!("{prefix} ui/src/b.rs finding");
            let allowance = format!("3 {a}\n3 {b}\n");
            let d = repo(
                &format!("slack-{}", kind.name()),
                &[(
                    &[
                        ("xtask/Cargo.toml", "x"),
                        (&store::rel("ui", kind), &allowance),
                        ("ui/src/a.rs", "before"),
                        ("ui/src/b.rs", "before"),
                    ],
                    "base",
                )],
            );
            git(&d, &["checkout", "-q", "-b", "change"]);
            commit(&d, &[("README", "unrelated change")], "change");
            let now = [("ui", r(&format!("2 {a}\n2 {b}\n")))];
            let check = |now: &[(&'static str, Ratchet)]| ratcheted(&Default::default(), &d, kind, now);
            assert!(check(&now).is_ok(), "untouched slack must pass");
            std::fs::write(d.join("ui/src/a.rs"), "after").expect("edit");
            assert_eq!(
                check(&now).expect_err("touched slack"),
                [format!("ui: {a} has unused ratchet slack: 3 -> 2; run cargo xtask ratchet")]
            );
            let tightened = store::load(&d, "ui", kind).expect("load").tightened(&now[0].1);
            store::save(&d, "ui", kind, &tightened).expect("tighten");
            assert!(check(&now).is_ok(), "tightened entries must pass");
            std::fs::remove_file(d.join("ui/src/a.rs")).expect("delete");
            let gone = [("ui", Ratchet::default())];
            assert_eq!(
                check(&gone).expect_err("removed finding"),
                [format!("ui: {a} has unused ratchet slack: 2 -> 0; run cargo xtask ratchet")]
            );
            store::save(&d, "ui", kind, &tightened.tightened(&gone[0].1)).expect("drop fixed entries");
            assert!(check(&gone).is_ok());
            std::fs::remove_dir_all(&d).expect("clean up");
        }
    }

    #[test]
    fn moved_entries_must_be_tightened_at_the_destination() {
        let d = repo(
            "moved-slack",
            &[(
                &[
                    ("xtask/Cargo.toml", "x"),
                    ("xtask/data/ui/size.txt", "90 fn ui/src/a.rs f\n"),
                    ("ui/src/a.rs", "before"),
                ],
                "base",
            )],
        );
        git(&d, &["checkout", "-q", "-b", "move"]);
        git(&d, &["mv", "ui/src/a.rs", "ui/src/b.rs"]);
        commit(&d, &[("xtask/data/moves.txt", "ui/src/a.rs -> ui/src/b.rs\n")], "move");
        let check = |n| ratcheted(&Default::default(), &d, Kind::Size, &[("ui", r(n))]);
        assert!(check("90 fn ui/src/b.rs f\n").is_ok(), "unchanged move must pass");
        let error = check("80 fn ui/src/b.rs f\n").expect_err("moved slack");
        assert_eq!(error, ["ui: fn ui/src/b.rs f has unused ratchet slack: 90 -> 80; run cargo xtask ratchet"]);
        crate::tighten::apply_moves_in(&d, &["ui"]).expect("--move");
        let now = r("80 fn ui/src/b.rs f\n");
        let tightened = store::load(&d, "ui", Kind::Size).expect("load").tightened(&now);
        store::save(&d, "ui", Kind::Size, &tightened).expect("tighten");
        assert!(check("80 fn ui/src/b.rs f\n").is_ok());
        std::fs::remove_dir_all(&d).expect("clean up");
    }

    // The point of moves: an entry under its new key passes against the base's old key, a larger one does not.
    #[test]
    fn a_moved_entry_is_checked_against_its_old_size() {
        let m = moves::parse("fn ui/src/ui.rs Ui::step -> fn ui/src/step.rs Ui::step\n").expect("moves");
        let base = moves::apply(&Ratchet::parse("465 fn ui/src/ui.rs Ui::step\n").expect("r"), &m).expect("apply");
        let same = Ratchet::parse("465 fn ui/src/step.rs Ui::step\n").expect("r");
        let grown = Ratchet::parse("470 fn ui/src/step.rs Ui::step\n").expect("r");
        assert!(base::widened(&base, &moves::apply(&same, &m).expect("apply")).is_empty());
        assert_eq!(base::widened(&base, &grown).len(), 1);
        // an unlisted move is a new entry
        let elsewhere = Ratchet::parse("465 fn ui/src/other.rs Ui::step\n").expect("r");
        assert_eq!(base::widened(&base, &elsewhere).len(), 1);
    }

    // The hole: `ratchet --move` emptied moves.txt, so the integration commit had renamed entries and no move that
    // explained them against a base without moves: the pre-commit gate failed on the integrator's own commit.
    #[test]
    fn the_integrated_moves_pass_the_gate_until_the_base_has_them() {
        let d = repo(
            "lifecycle",
            &[(&[("xtask/Cargo.toml", "x"), ("xtask/data/ui/size.txt", "465 fn ui/src/ui.rs Ui::step\n")], "base")],
        );
        let same = || [("ui", r("465 fn ui/src/step.rs Ui::step\n"))];
        let grown = || [("ui", r("466 fn ui/src/step.rs Ui::step\n"))];
        let gate = |what: &str| {
            assert!(
                ratcheted(&Default::default(), &d, Kind::Size, &same()).is_ok(),
                "{}: {:?}",
                what,
                ratcheted(&Default::default(), &d, Kind::Size, &same())
            );
            assert!(ratcheted(&Default::default(), &d, Kind::Size, &grown()).is_err(), "{}: growth passed", what);
        };
        git(&d, &["checkout", "-q", "-b", "step"]);
        commit(&d, &[("xtask/data/moves.txt", "fn ui/src/ui.rs Ui::step -> fn ui/src/step.rs Ui::step\n")], "move");
        gate("the branch");
        crate::tighten::apply_moves_in(&d, &["ui"]).expect("--move");
        assert_eq!(store::load(&d, "ui", Kind::Size).expect("load"), r("465 fn ui/src/step.rs Ui::step\n"));
        gate("the integration, before its commit");
        commit(&d, &[], "ratchet --move");
        gate("the integration commit");
        git(&d, &["checkout", "-q", "main"]);
        git(&d, &["merge", "-q", "--ff-only", "step"]);
        gate("main after the fast-forward");
        // the next step: the base has the renamed entry, so the move is no longer needed and goes
        git(&d, &["checkout", "-q", "-b", "next"]);
        commit(&d, &[("README", "next")], "next step");
        crate::tighten::apply_moves_in(&d, &["ui"]).expect("--move");
        assert!(load_moves(&d).expect("moves").is_empty());
        gate("the next step");
        std::fs::remove_dir_all(&d).expect("clean up");
    }

    // The hole: a new crate (then `ratchet: false`) dropped the allowances moved into it, so M2.1's pure move of Model
    // failed; `ratchet --move` saved only the ratcheted crates and lost them.
    #[test]
    fn a_new_crate_inherits_the_allowances_moved_into_it() {
        let d = repo(
            "newcrate",
            &[(
                &[
                    ("xtask/Cargo.toml", "x"),
                    ("xtask/data/ui/size.txt", "194 struct ui/src/model.rs Model\n90 fn ui/src/ui.rs f\n"),
                    ("xtask/data/ui/clippy.txt", "3 ui ui/src/model.rs clippy::x\n"),
                ],
                "base",
            )],
        );
        git(&d, &["checkout", "-q", "-b", "m2.1"]);
        commit(&d, &[("xtask/data/moves.txt", "ui/src/model.rs -> core/src/model.rs\n")], "move Model");
        let size = || [("ui", r("90 fn ui/src/ui.rs f\n")), ("core", r("194 struct core/src/model.rs Model\n"))];
        assert!(
            ratcheted(&Default::default(), &d, Kind::Size, &size()).is_ok(),
            "{:?}",
            ratcheted(&Default::default(), &d, Kind::Size, &size())
        );
        let clippy = [("ui", r("")), ("core", r("3 core core/src/model.rs clippy::x\n"))];
        assert!(ratcheted(&Default::default(), &d, Kind::Clippy, &clippy).is_ok());
        // otherwise the new crate starts clean
        let more = [("ui", r("")), ("core", r("194 struct core/src/model.rs Model\n61 fn core/src/x.rs g\n"))];
        assert!(ratcheted(&Default::default(), &d, Kind::Size, &more).is_err());
        // a move into a crate the gate does not cover fails and writes nothing
        let e = crate::tighten::apply_moves_in(&d, &["ui"]).expect_err("orphans");
        assert!(e.contains("struct core/src/model.rs Model"), "{}", e);
        assert_eq!(store::load(&d, "ui", Kind::Size).expect("load").0.len(), 2);
        crate::tighten::apply_moves_in(&d, &["ui", "core"]).expect("--move");
        assert_eq!(store::load(&d, "core", Kind::Size).expect("load"), r("194 struct core/src/model.rs Model\n"));
        assert_eq!(store::load(&d, "ui", Kind::Size).expect("load"), r("90 fn ui/src/ui.rs f\n"));
        commit(&d, &[], "ratchet --move");
        assert!(
            ratcheted(&Default::default(), &d, Kind::Size, &size()).is_ok(),
            "{:?}",
            ratcheted(&Default::default(), &d, Kind::Size, &size())
        );
        std::fs::remove_dir_all(&d).expect("clean up");
    }
}
