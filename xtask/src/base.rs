//! The protected base: the ratchet files as committed at the merge base with the integration branch. A change may
//! lower a ratchet or raise a coverage floor; it may not widen either. Without this, an edit of the ratchet file
//! would authorize the violation it lists.
//!
//! The base is `overhaul` (or `origin/overhaul` in a CI checkout); `OOS_GATE_BASE` overrides it in CI only. On the
//! base branch itself the merge base is `HEAD`, so the comparison takes `HEAD^1`. A git failure fails the check.

use crate::ratchet::Ratchet;
use crate::run;
use std::path::Path;

/// The ref to compare with: the CI override, else the first of `overhaul`, `origin/overhaul` that exists.
fn select(ci: bool, env: Option<&str>, exists: impl Fn(&str) -> bool) -> Result<String, String> {
    let named = env.filter(|_| ci);
    named
        .into_iter()
        .chain(["overhaul", "origin/overhaul"])
        .find(|r| exists(r))
        .map(str::to_string)
        .ok_or_else(|| "no base branch: fetch overhaul (CI: fetch-depth 0)".to_string())
}

fn git(ctx: &crate::run::Context, root: &Path, args: &[&str]) -> Result<String, String> {
    let out = run::capture(ctx, root, "git", args)?;
    if out.ok {
        Ok(out.stdout.trim().to_string())
    } else {
        Err(format!("git {}: {}", args.join(" "), out.stderr.trim()))
    }
}

/// The commit to compare with: the merge base of HEAD and `base`, or HEAD's first parent when HEAD is on the base.
pub fn commit(ctx: &crate::run::Context, root: &Path, base: &str) -> Result<String, String> {
    let mb = git(ctx, root, &["merge-base", "HEAD", base])?;
    if mb == git(ctx, root, &["rev-parse", "HEAD"])? {
        return git(ctx, root, &["rev-parse", "HEAD^1"]);
    }
    Ok(mb)
}

/// Whether `rel` exists at `commit`; an error for anything git cannot answer.
fn exists_at(ctx: &crate::run::Context, root: &Path, commit: &str, rel: &str) -> Result<bool, String> {
    Ok(!git(ctx, root, &["ls-tree", "--name-only", commit, "--", rel])?.is_empty())
}

/// A file at the base.
#[derive(Debug, PartialEq, Eq)]
pub enum At {
    Present(String),
    /// absent, while the gate exists at the base
    Absent,
    /// absent, and so is the gate (`xtask/Cargo.toml`): the change that brings the gate in
    NoGate,
}

/// `rel` at `commit`.
pub fn at(ctx: &crate::run::Context, root: &Path, commit: &str, rel: &str) -> Result<At, String> {
    if exists_at(ctx, root, commit, rel)? {
        return git(ctx, root, &["show", &format!("{}:{}", commit, rel)]).map(At::Present);
    }
    if exists_at(ctx, root, commit, "xtask/Cargo.toml")? {
        return Ok(At::Absent);
    }
    Ok(At::NoGate)
}

/// The text of `rel` at `commit`. None only when the gate is absent there too; a file the gate lost fails.
pub fn text_at(ctx: &crate::run::Context, root: &Path, commit: &str, rel: &str) -> Result<Option<String>, String> {
    match at(ctx, root, commit, rel)? {
        At::Present(t) => Ok(Some(t)),
        At::NoGate => Ok(None),
        At::Absent => Err(format!("{}: missing at the base {} although the gate exists there", rel, commit)),
    }
}

/// The base commit of the repository at `root`.
pub fn protected(ctx: &crate::run::Context, root: &Path) -> Result<String, String> {
    let env = std::env::var("OOS_GATE_BASE").ok();
    let exists = |r: &str| git(ctx, root, &["rev-parse", "--verify", "--quiet", r]).is_ok();
    let base = select(std::env::var_os("CI").is_some(), env.as_deref(), exists)?;
    commit(ctx, root, &base)
}

/// The text of `rel` at the protected base of the repository at `root`.
pub fn text(ctx: &crate::run::Context, root: &Path, rel: &str) -> Result<Option<String>, String> {
    text_at(ctx, root, &protected(ctx, root)?, rel)
}

/// Entries of `work` that the base does not allow: new keys and raised values.
pub fn widened(base: &Ratchet, work: &Ratchet) -> Vec<String> {
    work.0
        .iter()
        .filter_map(|(k, &w)| match base.0.get(k) {
            None => Some(format!("{}: added to the ratchet file (fix the finding instead)", k)),
            Some(&b) if w > b => Some(format!("{}: raised in the ratchet file from {} to {}", k, b, w)),
            Some(_) => None,
        })
        .collect()
}

/// Coverage floors of the base that `work` removed or lowered.
pub fn lowered(base: &Ratchet, work: &Ratchet) -> Vec<String> {
    base.0
        .iter()
        .filter_map(|(k, &b)| match work.0.get(k) {
            None => Some(format!("{}: coverage floor removed (base {})", k, b)),
            Some(&w) if w < b => Some(format!("{}: coverage floor lowered from {} to {}", k, b, w)),
            Some(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
