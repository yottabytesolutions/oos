// SPDX-License-Identifier: GPL-2.0-or-later
//! The single baseline transition authorized by issue #198 and ADR 0006.
//! Once the marker is at the protected base, ordinary shrink-only comparisons apply.

use crate::base::{self, At};
use crate::ratchet::Ratchet;
use crate::store::Kind;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug)]
pub enum BaselineError {
    Gate(String),
    Read(std::io::Error),
    Inventory,
}

impl std::fmt::Display for BaselineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Gate(message) => f.write_str(message),
            Self::Read(error) => write!(f, "style lint inventory: {error}"),
            Self::Inventory => f.write_str("cannot read baseline source inventory"),
        }
    }
}

impl std::error::Error for BaselineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(error) => Some(error),
            Self::Gate(_) | Self::Inventory => None,
        }
    }
}

pub const MARKER: &str = "xtask/data/style-lints.txt";

pub fn is_pending(ctx: &crate::run::Context, root: &Path, commit: &str) -> Result<bool, BaselineError> {
    Ok(root.join(MARKER).is_file()
        && matches!(base::at(ctx, root, commit, MARKER).map_err(BaselineError::Gate)?, At::Absent))
}

fn is_eligible(key: &str, kind: Kind, lints: &BTreeSet<&str>, old_lints: &BTreeSet<&str>) -> bool {
    match kind {
        Kind::Naming => true,
        Kind::Clippy => key.rsplit(' ').next().is_some_and(|lint| lints.contains(lint) && !old_lints.contains(lint)),
        _ => false,
    }
}

/// Existing lint limits are never widened. Only the measured, newly enabled rules on existing source files
/// receive their first baseline in this transition. New files must already meet the new rules.
pub fn extend_initial_baseline(
    ctx: &crate::run::Context,
    root: &Path,
    commit: &str,
    kind: Kind,
    base: &Ratchet,
    measured: &Ratchet,
) -> Result<Ratchet, BaselineError> {
    if !matches!(kind, Kind::Clippy | Kind::Naming) || !is_pending(ctx, root, commit)? {
        return Ok(base.clone());
    }
    let text = std::fs::read_to_string(root.join(MARKER)).map_err(BaselineError::Read)?;
    let lints = text.lines().filter(|line| line.starts_with("clippy::")).collect();
    let old_lints = base.0.keys().filter_map(|key| key.rsplit(' ').next()).collect();
    let output = crate::run::capture(ctx, root, "git", &["ls-tree", "-r", "--name-only", "HEAD"])
        .map_err(BaselineError::Gate)?;
    if !output.ok {
        return Err(BaselineError::Inventory);
    }
    let files: BTreeSet<_> = output.stdout.lines().collect();
    let mut allowed = base.clone();
    allowed.0.extend(measured.0.iter().filter_map(|(key, count)| {
        let file = key.split(' ').nth(1)?;
        (files.contains(file) && is_eligible(key, kind, &lints, &old_lints)).then(|| (key.clone(), *count))
    }));
    Ok(allowed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rollout_cannot_raise_an_existing_lint_or_an_unrelated_ratchet() {
        let new = BTreeSet::from(["clippy::use_self", "clippy::unwrap_used"]);
        let old = BTreeSet::from(["clippy::unwrap_used"]);
        assert!(is_eligible("ui ui/src/a.rs clippy::use_self", Kind::Clippy, &new, &old));
        assert!(!is_eligible("ui ui/src/a.rs clippy::unwrap_used", Kind::Clippy, &new, &old));
        assert!(!is_eligible("ui ui/src/a.rs clippy::other", Kind::Clippy, &new, &old));
        assert!(!is_eligible("file ui/src/a.rs", Kind::Size, &new, &old));
    }

    #[test]
    fn baseline_transition_is_closed_once_integrated_and_excludes_new_files() {
        use crate::testrepo::{commit, git, repo};
        let root = repo("style-rollout", &[(&[("xtask/Cargo.toml", "x"), ("ui/src/a.rs", "")], "base")]);
        git(&root, &["checkout", "-q", "-b", "style"]);
        let marker = "clippy::use_self\n";
        commit(&root, &[(MARKER, marker)], "style");
        let base = Ratchet::parse("1 ui ui/src/a.rs clippy::unwrap_used\n").unwrap();
        let work = Ratchet::parse(
            "2 ui ui/src/a.rs clippy::unwrap_used\n3 ui ui/src/a.rs clippy::use_self\n1 ui ui/src/new.rs clippy::use_self\n",
        )
        .unwrap();
        let ctx = crate::run::Context::default();
        let before = base::protected(&ctx, &root).unwrap();
        let initial = extend_initial_baseline(&ctx, &root, &before, Kind::Clippy, &base, &work).unwrap();
        assert_eq!(initial.0.len(), 2);
        assert_eq!(initial.0["ui ui/src/a.rs clippy::unwrap_used"], 1);
        let names = Ratchet::parse("1 name ui/src/a.rs Helper vague\n1 name ui/src/new.rs Helper vague\n").unwrap();
        assert_eq!(
            extend_initial_baseline(&ctx, &root, &before, Kind::Naming, &Ratchet::default(), &names).unwrap().0.len(),
            1
        );
        git(&root, &["checkout", "-q", "main"]);
        git(&root, &["merge", "-q", "--ff-only", "style"]);
        git(&root, &["checkout", "-q", "-b", "later"]);
        commit(&root, &[("README", "later")], "later");
        let after = base::protected(&ctx, &root).unwrap();
        assert!(!is_pending(&ctx, &root, &after).unwrap());
        assert_eq!(extend_initial_baseline(&ctx, &root, &after, Kind::Clippy, &base, &work).unwrap(), base);
        std::fs::remove_dir_all(root).unwrap();
    }
}
