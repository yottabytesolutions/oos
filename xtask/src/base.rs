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

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = run::capture(root, "git", args)?;
    if out.ok {
        Ok(out.stdout.trim().to_string())
    } else {
        Err(format!("git {}: {}", args.join(" "), out.stderr.trim()))
    }
}

/// The commit to compare with: the merge base of HEAD and `base`, or HEAD's first parent when HEAD is on the base.
pub fn commit(root: &Path, base: &str) -> Result<String, String> {
    let mb = git(root, &["merge-base", "HEAD", base])?;
    if mb == git(root, &["rev-parse", "HEAD"])? {
        return git(root, &["rev-parse", "HEAD^1"]);
    }
    Ok(mb)
}

/// Whether `rel` exists at `commit`; an error for anything git cannot answer.
fn exists_at(root: &Path, commit: &str, rel: &str) -> Result<bool, String> {
    Ok(!git(root, &["ls-tree", "--name-only", commit, "--", rel])?.is_empty())
}

/// The text of `rel` at `commit`. None only when the file is absent there and so is the gate (`xtask/Cargo.toml`):
/// the one change that brings the ratchets in. Absent while the gate exists fails.
pub fn text_at(root: &Path, commit: &str, rel: &str) -> Result<Option<String>, String> {
    if exists_at(root, commit, rel)? {
        return git(root, &["show", &format!("{}:{}", commit, rel)]).map(Some);
    }
    if exists_at(root, commit, "xtask/Cargo.toml")? {
        return Err(format!("{}: missing at the base {} although the gate exists there", rel, commit));
    }
    Ok(None)
}

/// The text of `rel` at the protected base of the repository at `root`.
pub fn text(root: &Path, rel: &str) -> Result<Option<String>, String> {
    let env = std::env::var("OOS_GATE_BASE").ok();
    let exists = |r: &str| git(root, &["rev-parse", "--verify", "--quiet", r]).is_ok();
    let base = select(std::env::var_os("CI").is_some(), env.as_deref(), exists)?;
    text_at(root, &commit(root, &base)?, rel)
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
