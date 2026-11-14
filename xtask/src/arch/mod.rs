//! Architecture boundaries: crate dependency direction and module rules, both from `xtask/data/architecture.txt`.

mod deps;
mod modules;
pub mod rules;

use crate::repo::{self, CRATES};
use std::path::Path;

/// The rules of the repository at `root`.
pub fn load(root: &Path) -> Result<rules::Rules, String> {
    let path = repo::data(root).join("architecture.txt");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {}", path.display(), e))?;
    rules::parse(&text).map_err(|e| format!("{}: {}", path.display(), e))
}

/// Every violation of the crate and module rules in the repository at `root`.
pub fn check(root: &Path) -> Result<Vec<String>, String> {
    let rules = load(root)?;
    let mut out = Vec::new();
    out.extend(deps::check(root, &rules)?);
    for c in CRATES {
        let forbid: Vec<&rules::Forbid> = rules.forbid.iter().filter(|f| f.krate == c.dir).collect();
        let layers: Vec<&rules::Layer> = rules.layers.iter().filter(|l| l.krate == c.dir).collect();
        if !forbid.is_empty() || !layers.is_empty() {
            out.extend(modules::check(root, c.dir, &forbid, &layers)?);
        }
    }
    Ok(out)
}
