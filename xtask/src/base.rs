//! The protected base: the ratchet files as committed at the merge base with the integration branch. A change may
//! lower a ratchet or raise a coverage floor; it may not widen either. Without this, an edit of the ratchet file
//! would authorize the violation it lists.

use crate::ratchet::Ratchet;
use crate::run;
use std::path::Path;

/// The branch the gate protects: `OOS_GATE_BASE`, else `overhaul`, else `origin/overhaul` (a CI checkout).
fn base_ref(root: &Path) -> Result<String, String> {
    let named = std::env::var("OOS_GATE_BASE").ok();
    let candidates = named.iter().map(String::as_str).chain(["overhaul", "origin/overhaul"]);
    for r in candidates {
        if run::capture(root, "git", &["rev-parse", "--verify", "--quiet", r]).is_ok_and(|o| o.ok) {
            return Ok(r.to_string());
        }
    }
    Err("no base branch: fetch overhaul (CI: fetch-depth 0) or set OOS_GATE_BASE".into())
}

/// The text of `rel` at the merge base of HEAD and the base branch; None when the file did not exist there (the
/// rule comes in with this change).
pub fn text(root: &Path, rel: &str) -> Result<Option<String>, String> {
    let b = base_ref(root)?;
    let mb = run::capture(root, "git", &["merge-base", "HEAD", &b])?;
    if !mb.ok {
        return Err(format!("git merge-base HEAD {}: {}", b, mb.stderr.trim()));
    }
    let spec = format!("{}:{}", mb.stdout.trim(), rel);
    let out = run::capture(root, "git", &["show", &spec])?;
    Ok(out.ok.then_some(out.stdout))
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
mod tests {
    use super::*;

    fn r(t: &str) -> Ratchet {
        Ratchet::parse(t).expect("ratchet")
    }

    // The hole: an edit of the ratchet file authorized a new or a grown finding.
    #[test]
    fn a_ratchet_file_may_only_shrink_against_the_base() {
        let base = r("5 fn a.rs f\n3 file a.rs\n");
        assert!(widened(&base, &r("4 fn a.rs f\n")).is_empty());
        assert_eq!(
            widened(&base, &r("6 fn a.rs f\n3 file a.rs\n")),
            ["fn a.rs f: raised in the ratchet file from 5 to 6"]
        );
        assert_eq!(
            widened(&base, &r("1 fn b.rs g\n")),
            ["fn b.rs g: added to the ratchet file (fix the finding instead)"]
        );
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
}
