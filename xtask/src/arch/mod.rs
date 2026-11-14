//! Architecture boundaries: crate dependency direction and module rules, both from `xtask/data/architecture.txt`.

mod deps;
mod modules;
mod resolve;
pub mod rules;
mod spelled;

use crate::repo::{self, CRATES};
use crate::sources;
use crate::workspace::Metadata;
use std::path::Path;

/// The rules of the repository at `root`.
pub fn load(root: &Path) -> Result<rules::Rules, String> {
    let path = repo::data(root).join("architecture.txt");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    rules::parse(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

/// Every violation of the crate and module rules in the workspace `meta` at `root`, and members without a gate
/// entry.
pub fn check(root: &Path, meta: &Metadata) -> Result<Vec<String>, String> {
    let rules = load(root)?;
    let mut out = meta.unlisted();
    out.extend(deps::check(meta, &rules));
    for c in CRATES {
        let cr = modules::CrateRules {
            forbid: rules.forbid.iter().filter(|f| f.krate == c.dir).collect(),
            layers: rules.layers.iter().filter(|l| l.krate == c.dir).collect(),
            confines: rules.confines.iter().filter(|x| x.krate == c.dir).collect(),
            deps: meta.member(c.dir).map_or_else(Default::default, |p| {
                p.dependencies.iter().map(|d| (d.code_name(), d.canonical())).collect()
            }),
        };
        if !cr.forbid.is_empty() || !cr.layers.is_empty() || !cr.confines.is_empty() {
            out.extend(modules::check(&sources::of_dir(meta, c.dir)?, &cr));
        }
    }
    Ok(out)
}
