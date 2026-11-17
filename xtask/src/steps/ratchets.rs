//! The comparison every ratchet step shares: the current entries of each crate against its ratchet file, after the
//! planned moves, with the file itself checked against the base.

use super::{outcome, Outcome};
use crate::base::{self, At};
use crate::moves::{self, Move};
use crate::ratchet::Ratchet;
use crate::rebaseline;
use crate::store::{self, Kind};
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

/// The allowed entries of every ratcheted crate for `kind`, moves applied; their base, moves applied and widened by
/// the rebaseline commits of upstream intake (adr/0018), when there is one.
fn allowed(
    ctx: &crate::run::Context,
    root: &Path,
    kind: Kind,
    crates: &[&str],
    commit: &str,
) -> Result<(Ratchet, Option<Ratchet>), String> {
    let moves = all_moves(ctx, root, Some(commit))?;
    let admitted = rebaseline::admitted(ctx, root, commit)?;
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
    // the admitted entries carry their current keys: they join the base after its moves
    let base = base.map(|b| moves::apply(&b, &moves)).transpose()?.map(|mut b| {
        for k in crates {
            if let Some(more) = admitted.get(&store::rel(k, kind)) {
                rebaseline::widen(&mut b, more);
            }
        }
        b
    });
    Ok((moves::apply(&work, &moves)?, base))
}

/// The entries of `work` wider than `base` that pending upstream code explains (fast gate only; adr/0018).
fn held_widening(base: Ratchet, work: &Ratchet, pending: &rebaseline::Changed) -> Ratchet {
    let grown = rebaseline::widened_keys(&base, work);
    Ratchet(grown.0.into_iter().filter(|(k, _)| pending.covers(k)).collect())
}

/// The fast gate's pending entries (adr/0018): the failures of `v` that upstream code not yet rebaselined explains.
fn split_pending(v: &crate::ratchet::Verdict, pending: &rebaseline::Changed) -> (Vec<String>, usize) {
    let kept = crate::ratchet::Verdict {
        grown: v.grown.iter().filter(|g| !pending.covers(&g.0)).cloned().collect(),
        new: v.new.iter().filter(|n| !pending.covers(&n.0)).cloned().collect(),
        shrunk: Vec::new(),
    };
    let n = v.grown.len() + v.new.len() - kept.grown.len() - kept.new.len();
    (kept.failures(), n)
}

/// Compares `now` of every crate (`(crate, entries)`) with the ratchet files of `kind`, moves applied. A crate allows
/// what its files list and what the moves bring into it; a new crate without files starts clean.
pub fn ratcheted(ctx: &crate::run::Context, root: &Path, kind: Kind, now: &[(&'static str, Ratchet)]) -> Outcome {
    let crates: Vec<&str> = now.iter().map(|n| n.0).collect();
    let commit = base::protected(ctx, root).map_err(|e| vec![e])?;
    let (work, base) = allowed(ctx, root, kind, &crates, &commit).map_err(|e| vec![e])?;
    let pending = match ctx.is_fast() {
        true => rebaseline::pending(ctx, root, &commit).map_err(|e| vec![e])?,
        false => rebaseline::Changed::default(),
    };
    if let Some(b) = &base {
        // the fast gate also holds a rebaseline being committed: the entries it widens for pending upstream code
        let held = held_widening(b.clone(), &work, &pending);
        let mut b = b.clone();
        rebaseline::widen(&mut b, &held);
        let widened = base::widened(&b, &work);
        if !widened.is_empty() {
            return Err(widened.into_iter().map(|w| format!("{} ratchet: {}", kind.name(), w)).collect());
        }
    }
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for (krate, n) in now {
        let v = store::of_crate(&work, krate).compare(n);
        let (failures, held) = split_pending(&v, &pending);
        bad.extend(failures.into_iter().map(|f| format!("{}: {}", krate, f)));
        let total: u64 = n.0.values().sum();
        let shrunk = if v.shrunk.is_empty() {
            String::new()
        } else {
            format!(", {} improved (cargo xtask ratchet)", v.shrunk.len())
        };
        let held = if held == 0 { String::new() } else { format!(", {} pending rebaseline (adr/0018)", held) };
        ok.push(format!("{}: {} entries, {} total{}{}", krate, n.0.len(), total, shrunk, held));
    }
    outcome(ok, bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The fast gate holds a rebaseline being committed only for upstream code: our own file stays widened.
    #[test]
    fn the_fast_gate_holds_only_upstream_widening() {
        let pending = rebaseline::Changed::of_files(["ui/src/up.rs".to_string()].into());
        let base = Ratchet::parse("400 file ui/src/ours.rs\n").expect("r");
        let work = Ratchet::parse("450 file ui/src/ours.rs\n70 fn ui/src/up.rs f\n").expect("r");
        let held = held_widening(base, &work, &pending);
        assert_eq!(held.0.keys().collect::<Vec<_>>(), ["fn ui/src/up.rs f"]);
    }
    use crate::testrepo::{commit, git, repo};

    fn r(t: &str) -> Ratchet {
        Ratchet::parse(t).expect("r")
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
