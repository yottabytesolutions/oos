//! `cargo xtask ratchet`: writes the ratchet files back after an improvement. Entries only shrink or go; the
//! coverage floors only rise. `--move` applies the planned moves (the integrator, at merge) and keeps those the base
//! still needs in the moves file. `--init` writes the
//! files from the current state, only where the base has no gate yet.

use crate::moves::{self, Move};
use crate::ratchet::Ratchet;
use crate::repo::CRATES;
use crate::store::{self, Kind};
use crate::{clippy, steps};
use std::collections::BTreeMap;
use std::path::Path;

const COVERAGE_HEAD: &str = "Line coverage floor per crate in tenths of a percent: <floor> <crate>.\n\
Written by `cargo xtask ratchet` (docs/development.md). Floors may only rise.";

/// What `cargo xtask ratchet` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub init: bool,
    pub coverage: bool,
    pub moves: bool,
}

/// The current entries of every crate, by crate and kind.
fn current(root: &Path) -> Result<BTreeMap<(&'static str, &'static str), Ratchet>, String> {
    let rules = crate::arch::load(root)?;
    let mut out = BTreeMap::new();
    for (c, m) in steps::measured(&Default::default(), root)? {
        let f = clippy::run(&Default::default(), root, c.dir, c.target)?;
        if !f.errors.is_empty() {
            return Err(format!("{}: does not build:\n{}", c.dir, f.errors.join("\n")));
        }
        let unsafes = steps::unsafe_now(&m, &rules.unsafe_paths);
        let statics = Ratchet(m.statics.0.into_iter().filter(|(k, _)| !rules.statics.contains(k)).collect());
        for (kind, r) in [
            (Kind::Clippy, f.counts),
            (Kind::Size, m.sizes),
            (Kind::Statics, statics),
            (Kind::Unsafe, unsafes),
            (Kind::Allow, m.allows),
            (Kind::Known, m.known),
        ] {
            out.insert((c.dir, kind.name()), r);
        }
    }
    Ok(out)
}

/// The coverage floors after `now`: raised to the current value, never lowered.
fn raised(floor: &Ratchet, now: &Ratchet) -> Ratchet {
    Ratchet(now.0.iter().map(|(k, &n)| (k.clone(), n.max(floor.0.get(k).copied().unwrap_or(0)))).collect())
}

/// `--init` only where the base has no gate: over an existing ratchet it would list anything.
fn refuse_init(root: &Path) -> Result<(), String> {
    let commit = crate::base::protected(&Default::default(), root)?;
    if store::at_base(&Default::default(), root, &commit, "hal", Kind::Size)?.is_some() {
        return Err("the base has the gate: --init only brings the ratchets in".into());
    }
    Ok(())
}

/// The kind with `name`.
fn kind(name: &str) -> Kind {
    Kind::ALL.into_iter().find(|k| k.name() == name).unwrap_or(Kind::Size)
}

/// Writes `r`; a crate without the file and without entries keeps having none.
fn save(root: &Path, krate: &str, k: Kind, r: &Ratchet) -> Result<(), String> {
    if r.0.is_empty() && !store::exists(root, krate, k) {
        return Ok(());
    }
    store::save(root, krate, k, r)
}

/// The ratchets of `kind` over `crates` in one set, from the working tree or (`commit`) the base.
fn combined(root: &Path, crates: &[&str], k: Kind, commit: Option<&str>) -> Result<Ratchet, String> {
    let mut all = Ratchet::default();
    for c in crates {
        let r = match commit {
            Some(b) => store::at_base(&Default::default(), root, b, c, k)?.unwrap_or_default(),
            None => store::load(root, c, k)?,
        };
        all.0.extend(r.0);
    }
    Ok(all)
}

/// The moves the base still needs: those that rename an entry of its ratchets, in order. The others are applied
/// there and change nothing.
fn needed(root: &Path, crates: &[&str], moves: &[Move]) -> Result<Vec<Move>, String> {
    let commit = crate::base::protected(&Default::default(), root)?;
    let mut keep = vec![false; moves.len()];
    for k in Kind::ALL {
        let mut cur = combined(root, crates, k, Some(&commit))?;
        for (i, m) in moves.iter().enumerate() {
            keep[i] |= moves::pending(&cur, m);
            cur = moves::apply(&cur, std::slice::from_ref(m))?;
        }
    }
    Ok(moves.iter().zip(keep).filter(|(_, k)| *k).map(|(m, _)| m.clone()).collect())
}

/// Writes the moves file with `moves`.
fn write_moves(root: &Path, moves: &[Move]) -> Result<(), String> {
    let mut text: String = moves::HEAD.lines().map(|l| format!("# {}\n", l)).collect();
    text.extend(moves.iter().map(|m| format!("{} -> {}\n", m.old, m.new)));
    std::fs::write(root.join("xtask/data/moves.txt"), text).map_err(|e| format!("xtask/data/moves.txt: {}", e))
}

/// Applies the planned moves to the ratchet files of `crates`. A moved entry that lands in none of them fails before
/// anything is written. The moves file keeps the moves the base still needs: they explain the renamed entries until
/// the base has them.
pub(crate) fn apply_moves_in(root: &Path, crates: &[&str]) -> Result<(), String> {
    let moves = steps::load_moves(root)?;
    let (mut out, mut lost) = (Vec::new(), Vec::new());
    for k in Kind::ALL {
        let moved = moves::apply(&combined(root, crates, k, None)?, &moves)?;
        let per: Vec<(&str, Ratchet)> = crates.iter().map(|c| (*c, store::of_crate(&moved, c))).collect();
        lost.extend(moved.0.into_keys().filter(|key| !per.iter().any(|(_, r)| r.0.contains_key(key))));
        out.extend(per.into_iter().map(|(c, r)| (c, k, r)));
    }
    if !lost.is_empty() {
        return Err(format!("moved into no crate of the gate (CRATES in xtask/src/repo.rs): {}", lost.join(", ")));
    }
    let keep = needed(root, crates, &moves)?;
    for (c, k, r) in &out {
        save(root, c, *k, r)?;
    }
    write_moves(root, &keep)?;
    println!("applied {} moves, {} kept for the base", moves.len(), keep.len());
    Ok(())
}

/// Whether a planned move still names an entry of the ratchet files of `crates`: `--move` has not run.
fn moves_pending(root: &Path, crates: &[&str]) -> Result<bool, String> {
    let moves = steps::load_moves(root)?;
    for k in Kind::ALL {
        let r = combined(root, crates, k, None)?;
        if moves.iter().any(|m| moves::pending(&r, m)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Runs `cargo xtask ratchet` with `o`.
pub fn run(root: &Path, o: Options) -> Result<(), String> {
    let crates: Vec<&str> = CRATES.iter().map(|c| c.dir).collect();
    if o.moves {
        return apply_moves_in(root, &crates);
    }
    if moves_pending(root, &crates)? {
        return Err("moves are planned (xtask/data/moves.txt): the integrator applies them with --move at merge; \
                    tighten after that"
            .into());
    }
    if o.init {
        refuse_init(root)?;
    }
    for ((c, k), now) in current(root)? {
        let k = kind(k);
        let r = if o.init { now } else { store::load(root, c, k)?.tightened(&now) };
        save(root, c, k, &r)?;
        println!("{}: {} entries", store::rel(c, k), r.0.len());
    }
    if o.coverage {
        let path = steps::ratchet_path(root, "coverage.txt");
        let now = steps::coverage_now(&Default::default(), root).map_err(|e| e.join("\n"))?;
        let floor = if o.init { Ratchet::default() } else { Ratchet::load(&path)? };
        std::fs::write(&path, raised(&floor, &now).text(COVERAGE_HEAD))
            .map_err(|e| format!("{}: {}", path.display(), e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floors_only_rise() {
        let floor = Ratchet::parse("900 ui\n800 hal\n").expect("r");
        let now = Ratchet::parse("850 ui\n820 hal\n700 dsp\n").expect("r");
        assert_eq!(raised(&floor, &now), Ratchet::parse("900 ui\n820 hal\n700 dsp\n").expect("r"));
    }

    #[test]
    fn every_kind_has_a_name() {
        assert!(Kind::ALL.into_iter().all(|k| kind(k.name()) == k));
    }

    // Plain `ratchet` waits for `--move`; after it, the kept moves no longer hold it up.
    #[test]
    fn tightening_waits_only_for_unapplied_moves() {
        use crate::testrepo::{commit, git, repo};
        let d =
            repo("pending", &[(&[("xtask/Cargo.toml", "x"), ("xtask/data/ui/size.txt", "9 fn ui/src/a.rs f\n")], "b")]);
        git(&d, &["checkout", "-q", "-b", "step"]);
        commit(&d, &[("xtask/data/moves.txt", "ui/src/a.rs -> ui/src/b.rs\n")], "move");
        assert!(moves_pending(&d, &["ui"]).expect("pending"));
        apply_moves_in(&d, &["ui"]).expect("--move");
        assert!(!moves_pending(&d, &["ui"]).expect("pending"));
        assert_eq!(steps::load_moves(&d).expect("moves").len(), 1);
        std::fs::remove_dir_all(&d).expect("clean up");
    }
}
