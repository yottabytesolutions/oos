// SPDX-License-Identifier: GPL-2.0-or-later
//! The steps over the walked sources: size limits, mutable statics and `unsafe`.

use super::{ratcheted, Outcome};
use crate::ratchet::Ratchet;
use crate::repo::{Crate, CRATES};
use crate::store::Kind;
use crate::{arch, size, sources, workspace};
use std::path::Path;

/// Every crate with what the walk of its sources found.
pub fn measured(ctx: &crate::run::Context, root: &Path) -> Result<Vec<(Crate, size::Measured)>, String> {
    let meta = workspace::load(ctx, root)?;
    CRATES.iter().map(|c| Ok((*c, size::measure(c.dir, &sources::of_dir(&meta, c.dir)?)))).collect()
}

pub fn size(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let now: Vec<_> = measured(ctx, root).map_err(|e| vec![e])?.into_iter().map(|(c, m)| (c.dir, m.sizes)).collect();
    ratcheted(ctx, root, Kind::Size, &now)
}

/// Exceptions to `clippy::disallowed_methods`: their count per file may only fall.
pub fn allows(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let now: Vec<_> = measured(ctx, root).map_err(|e| vec![e])?.into_iter().map(|(c, m)| (c.dir, m.allows)).collect();
    ratcheted(ctx, root, Kind::Allow, &now)
}

/// Whether task `n` is open: `docs/tasks/NNNN-*.md` exists outside `done/`.
fn open_task(root: &Path, n: &str) -> bool {
    std::fs::read_dir(root.join("docs").join("tasks"))
        .map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().starts_with(&format!("{}-", n))))
        .unwrap_or(false)
}

/// Known failures: each names an open task, and the list may only shrink.
pub fn known(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let mut now = Vec::new();
    let mut bad = Vec::new();
    for (c, m) in measured(ctx, root).map_err(|e| vec![e])? {
        bad.extend(m.malformed.iter().map(|x| format!("{}: ignore reason: {}", c.dir, x)));
        for k in m.known.0.keys() {
            let task = k.rsplit(' ').next().unwrap_or("");
            if !open_task(root, task) {
                bad.push(format!("{}: task {} is not open in docs/tasks", k, task));
            }
        }
        now.push((c.dir, m.known));
    }
    if !bad.is_empty() {
        return Err(bad);
    }
    ratcheted(ctx, root, Kind::Known, &now)
}

/// `unsafe` outside the allowed paths of a crate (`unsafe` lines in architecture.txt).
pub fn unsafe_now(m: &size::Measured, allowed: &[String]) -> Ratchet {
    let ok = |k: &str| k.split_once(' ').is_some_and(|(_, f)| allowed.iter().any(|p| size::unsafety::under(f, p)));
    Ratchet(m.unsafes.0.iter().filter(|(k, _)| !ok(k)).map(|(k, v)| (k.clone(), *v)).collect())
}

/// `unsafe` only in the allowed paths (architecture.txt) and the listed old places (unsafe.txt).
pub fn unsafety(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let rules = arch::load(root).map_err(|e| vec![e])?;
    let now: Vec<_> = measured(ctx, root)
        .map_err(|e| vec![e])?
        .into_iter()
        .map(|(c, m)| (c.dir, unsafe_now(&m, &rules.unsafe_paths)))
        .collect();
    ratcheted(ctx, root, Kind::Unsafe, &now)
}

/// Mutable statics: only metrics (allowlist in architecture.txt) and the listed old ones (statics.txt).
pub fn statics(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let rules = arch::load(root).map_err(|e| vec![e])?;
    let mut now = Vec::new();
    let mut all = std::collections::BTreeSet::new();
    for (c, m) in measured(ctx, root).map_err(|e| vec![e])? {
        let mut s = m.statics;
        all.extend(s.0.keys().cloned());
        s.0.retain(|k, _| !rules.statics.contains(k));
        now.push((c.dir, s));
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
    ratcheted(ctx, root, Kind::Statics, &now)
}
