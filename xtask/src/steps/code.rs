//! The steps over the walked sources: size limits, mutable statics and `unsafe`.

use super::{ratcheted, Outcome};
use crate::ratchet::Ratchet;
use crate::repo::{Crate, CRATES};
use crate::{arch, size, sources, workspace};
use std::path::Path;

/// Every crate with what the walk of its sources found.
pub fn measured(root: &Path) -> Result<Vec<(Crate, size::Measured)>, String> {
    let meta = workspace::load(root)?;
    CRATES.iter().map(|c| Ok((*c, size::measure(c.dir, &sources::of_dir(&meta, c.dir)?)))).collect()
}

pub fn size(root: &Path) -> Outcome {
    let now: Vec<_> =
        measured(root).map_err(|e| vec![e])?.into_iter().map(|(c, m)| (c.dir, c.ratchet, m.sizes)).collect();
    ratcheted(root, "size.txt", &now)
}

/// `unsafe` outside the allowed paths of a crate (`unsafe` lines in architecture.txt).
pub fn unsafe_now(m: &size::Measured, allowed: &[String]) -> Ratchet {
    let ok = |k: &str| k.split_once(' ').is_some_and(|(_, f)| allowed.iter().any(|p| size::unsafety::under(f, p)));
    Ratchet(m.unsafes.0.iter().filter(|(k, _)| !ok(k)).map(|(k, v)| (k.clone(), *v)).collect())
}

/// `unsafe` only in the allowed paths (architecture.txt) and the listed old places (unsafe.txt).
pub fn unsafety(root: &Path) -> Outcome {
    let rules = arch::load(root).map_err(|e| vec![e])?;
    let now: Vec<_> = measured(root)
        .map_err(|e| vec![e])?
        .into_iter()
        .map(|(c, m)| (c.dir, c.ratchet, unsafe_now(&m, &rules.unsafe_paths)))
        .collect();
    ratcheted(root, "unsafe.txt", &now)
}

/// Mutable statics: only metrics (allowlist in architecture.txt) and the listed old ones (statics.txt).
pub fn statics(root: &Path) -> Outcome {
    let rules = arch::load(root).map_err(|e| vec![e])?;
    let mut now = Vec::new();
    let mut all = std::collections::BTreeSet::new();
    for (c, m) in measured(root).map_err(|e| vec![e])? {
        let mut s = m.statics;
        all.extend(s.0.keys().cloned());
        s.0.retain(|k, _| !rules.statics.contains(k));
        now.push((c.dir, c.ratchet, s));
    }
    let stale: Vec<String> = rules
        .statics
        .iter()
        .filter(|k| !all.contains(*k))
        .map(|k| format!("{}: listed as metrics, not found", k))
        .collect();
    if !stale.is_empty() {
        return Err(stale);
    }
    ratcheted(root, "statics.txt", &now)
}
