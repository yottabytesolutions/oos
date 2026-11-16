//! Upstream intake (adr/0018): the one way a ratchet entry may be added or raised. A rebaseline commit between the
//! protected base and HEAD admits entries when it
//!
//! - touches only `xtask/data/**`,
//! - carries the trailer `Ratchet-Rebaseline: upstream <sha>`,
//! - has a merge of that upstream commit (or an ancestor of it) as an ancestor inside the integration, and
//! - adds or raises only keys of code upstream changed: a key whose file one of those merges changed, a key that a
//!   move in the base's `xtask/data/moves.txt` carried out of such a file, or a crate key (`impls ui Ui`) of a crate
//!   those merges changed.
//!
//! Anything else in such a commit fails the gate. A commit without the trailer admits nothing. Until the rebaseline
//! commit exists, the fast gate reports the findings such keys would cover as pending (`pending`); the full gate
//! admits nothing without it.

use crate::moves::Move;
use crate::ratchet::Ratchet;
use crate::run;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[cfg(test)]
mod tests;

/// The trailer that marks a rebaseline commit.
pub const TRAILER: &str = "Ratchet-Rebaseline";

/// Admitted entries per ratchet file (`xtask/data/<crate>/<kind>.txt`).
pub type Admitted = BTreeMap<String, Ratchet>;

/// What upstream changed: files, and the keys moves carried out of them.
#[derive(Debug, Default)]
pub struct Changed {
    pub files: BTreeSet<String>,
    moved: Vec<Move>,
}

impl Changed {
    /// Whether ratchet key `k` belongs to code upstream changed.
    pub fn covers(&self, k: &str) -> bool {
        let words: Vec<&str> = k.split(' ').collect();
        let carried = |m: &Move| {
            let from_changed = m.old.split(' ').find(|w| w.contains('/')).is_some_and(|f| self.files.contains(f));
            from_changed && (m.new == k || (!m.new.contains(' ') && words.contains(&m.new.as_str())))
        };
        let direct = match words.iter().find(|w| w.contains('/')) {
            Some(f) => self.files.contains(*f),
            None => words.get(1).is_some_and(|c| self.files.iter().any(|f| f.starts_with(&format!("{}/", c)))),
        };
        direct || self.moved.iter().any(carried)
    }
}

fn git(ctx: &run::Context, root: &Path, args: &[&str]) -> Result<String, String> {
    let out = run::capture(ctx, root, "git", args)?;
    if out.ok {
        Ok(out.stdout.trim().to_string())
    } else {
        Err(format!("git {}: {}", args.join(" "), out.stderr.trim()))
    }
}

fn is_ancestor(ctx: &run::Context, root: &Path, a: &str, b: &str) -> Result<bool, String> {
    Ok(run::capture(ctx, root, "git", &["merge-base", "--is-ancestor", a, b])?.ok)
}

fn short(c: &str) -> &str {
    c.get(..8).unwrap_or(c)
}

/// The moves of the base's moves file: only moves made and reviewed before the integration count.
fn base_moves(ctx: &run::Context, root: &Path, base: &str) -> Result<Vec<Move>, String> {
    match crate::base::at(ctx, root, base, "xtask/data/moves.txt")? {
        crate::base::At::Present(t) => crate::moves::parse(&t),
        _ => Ok(Vec::new()),
    }
}

/// An upstream merge: a merge after `base` that changed the tree (a bridge with strategy ours does not) and whose
/// own merge base is not in `base` (a task branch starts from the base; upstream does not). Returns its upstream
/// parent and the files it brought.
fn upstream_merge(
    ctx: &run::Context,
    root: &Path,
    base: &str,
    ours: &str,
    theirs: &str,
) -> Result<Option<BTreeSet<String>>, String> {
    let mb = git(ctx, root, &["merge-base", ours, theirs])?;
    if is_ancestor(ctx, root, theirs, base)? || is_ancestor(ctx, root, &mb, base)? {
        return Ok(None);
    }
    Ok(Some(git(ctx, root, &["diff", "--name-only", &mb, theirs])?.lines().map(str::to_string).collect()))
}

/// The upstream merges in `base..c`: (upstream parent, files).
fn merges(ctx: &run::Context, root: &Path, base: &str, c: &str) -> Result<Vec<(String, BTreeSet<String>)>, String> {
    let mut out = Vec::new();
    for m in git(ctx, root, &["rev-list", "--merges", &format!("{}..{}", base, c)])?.lines() {
        let tree = |r: &str| git(ctx, root, &["rev-parse", &format!("{}^{{tree}}", r)]);
        if tree(m)? == tree(&format!("{}^1", m))? {
            continue;
        }
        let p2 = git(ctx, root, &["rev-parse", &format!("{}^2", m)])?;
        if let Some(files) = upstream_merge(ctx, root, base, &format!("{}^1", m), &p2)? {
            out.push((p2, files));
        }
    }
    Ok(out)
}

/// The rebaseline commits in `base..HEAD`: (commit, upstream sha).
fn rebaselines(ctx: &run::Context, root: &Path, base: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    let format = format!("--format=%H %(trailers:key={},valueonly,separator=%x20)", TRAILER);
    for line in git(ctx, root, &["log", "--reverse", &format, &format!("{}..HEAD", base)])?.lines() {
        let (c, value) = line.split_once(' ').unwrap_or((line, ""));
        if value.trim().is_empty() {
            continue;
        }
        let sha = value
            .trim()
            .strip_prefix("upstream ")
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.contains(char::is_whitespace))
            .ok_or_else(|| format!("rebaseline {}: trailer {:?} is not 'upstream <sha>'", short(c), value.trim()))?;
        let up = git(ctx, root, &["rev-parse", "--verify", "--quiet", &format!("{}^{{commit}}", sha)])
            .map_err(|_| format!("rebaseline {}: upstream commit {} not found", short(c), sha))?;
        out.push((c.to_string(), up));
    }
    Ok(out)
}

/// What the rebaseline commits between `base` and HEAD admit; an error for a rebaseline commit that breaks a rule.
pub fn admitted(ctx: &run::Context, root: &Path, base: &str) -> Result<Admitted, String> {
    let moved = base_moves(ctx, root, base)?;
    let mut all = Admitted::new();
    for (c, up) in rebaselines(ctx, root, base)? {
        let mut changed = Changed { files: BTreeSet::new(), moved: moved.clone() };
        for (p2, files) in merges(ctx, root, base, &c)? {
            if is_ancestor(ctx, root, &p2, &up)? {
                changed.files.extend(files);
            }
        }
        if changed.files.is_empty() {
            return Err(format!("rebaseline {}: no merge of upstream {} before it", short(&c), short(&up)));
        }
        for (file, r) in commit_admits(ctx, root, &c, &changed)? {
            widen(all.entry(file).or_default(), &r);
        }
    }
    Ok(all)
}

/// The entries commit `c` adds or raises, per ratchet file; an error when `c` touches anything but `xtask/data/` or
/// widens a key upstream did not change.
fn commit_admits(ctx: &run::Context, root: &Path, c: &str, changed: &Changed) -> Result<Admitted, String> {
    let parents = git(ctx, root, &["rev-list", "--parents", "-n", "1", c])?;
    if parents.split(' ').count() != 2 {
        return Err(format!("rebaseline {}: a rebaseline is a plain commit, not a merge", short(c)));
    }
    let touched = git(ctx, root, &["diff-tree", "--no-commit-id", "--name-only", "-r", c])?;
    if let Some(f) = touched.lines().find(|f| !f.starts_with("xtask/data/")) {
        return Err(format!("rebaseline {}: touches {} (only xtask/data/** allowed)", short(c), f));
    }
    let mut out = Admitted::new();
    for f in touched.lines().filter(|f| f.ends_with(".txt") && *f != "xtask/data/moves.txt") {
        let grown = widened_keys(&ratchet_at(ctx, root, &format!("{}^1", c), f)?, &ratchet_at(ctx, root, c, f)?);
        if let Some(k) = grown.0.keys().find(|k| !changed.covers(k)) {
            return Err(format!("rebaseline {}: {} widens {:?}, not code upstream changed", short(c), f, k));
        }
        if !grown.0.is_empty() {
            out.insert(f.to_string(), grown);
        }
    }
    Ok(out)
}

/// The ratchet `rel` at `commit`; empty when the file is not there.
fn ratchet_at(ctx: &run::Context, root: &Path, commit: &str, rel: &str) -> Result<Ratchet, String> {
    match crate::base::at(ctx, root, commit, rel)? {
        crate::base::At::Present(t) => Ratchet::parse(&t).map_err(|e| format!("{} at {}: {}", rel, short(commit), e)),
        _ => Ok(Ratchet::default()),
    }
}

/// The entries of `after` that are new or larger than in `before`.
pub fn widened_keys(before: &Ratchet, after: &Ratchet) -> Ratchet {
    let grown = after.0.iter().filter(|(k, v)| before.0.get(*k).is_none_or(|b| *v > b));
    Ratchet(grown.map(|(k, v)| (k.clone(), *v)).collect())
}

/// Adds `more` to `base`, the larger value winning.
pub fn widen(base: &mut Ratchet, more: &Ratchet) {
    for (k, v) in &more.0 {
        let slot = base.0.entry(k.clone()).or_insert(*v);
        *slot = (*slot).max(*v);
    }
}

/// For the fast gate: what upstream changed in the merges that no rebaseline commit covers yet, including a merge in
/// progress (`MERGE_HEAD`). Empty once every upstream merge has its rebaseline.
pub fn pending(ctx: &run::Context, root: &Path, base: &str) -> Result<Changed, String> {
    let covered = rebaselines(ctx, root, base)?;
    let mut changed = Changed { files: BTreeSet::new(), moved: base_moves(ctx, root, base)? };
    for (p2, files) in merges(ctx, root, base, "HEAD")? {
        let mut done = false;
        for (_, up) in &covered {
            done |= is_ancestor(ctx, root, &p2, up)?;
        }
        if !done {
            changed.files.extend(files);
        }
    }
    if let Ok(mh) = git(ctx, root, &["rev-parse", "--verify", "--quiet", "MERGE_HEAD"]) {
        if !is_ancestor(ctx, root, &mh, "HEAD")? {
            changed.files.extend(upstream_merge(ctx, root, base, "HEAD", &mh)?.unwrap_or_default());
        }
    }
    Ok(changed)
}
