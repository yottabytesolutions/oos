//! `cargo xtask check`: execute the inventory and preserve incomplete evidence.
use crate::inventory;
#[cfg(test)]
use crate::inventory::Selection;
use crate::report::{self, Step};
use crate::steps;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub fast: bool,
    pub coverage: bool,
}

/// G1/0020's Rust subset, retained for their independent dispatch fixtures.
#[cfg(test)]
pub(crate) fn plan(o: Options) -> Vec<(&'static str, inventory::Runner)> {
    inventory::CHECKS
        .iter()
        .filter(|c| matches!(c.selection, Selection::Fast | Selection::Rust | Selection::Coverage))
        .filter(|c| inventory::selected(c, o, false))
        .map(|c| (c.name, c.run))
        .collect()
}

pub(crate) fn formatting_and_pins(ctx: &crate::run::Context, root: &Path) -> steps::Outcome {
    let pins = crate::doctor::syntax(ctx, root);
    let fmt = steps::fmt(ctx, root);
    match (pins, fmt) {
        (Ok(mut a), Ok(b)) => {
            a.extend(b);
            Ok(a)
        }
        (Err(mut a), Err(b)) => {
            a.extend(b);
            Err(a)
        }
        (Err(e), _) | (_, Err(e)) => Err(e),
    }
}

pub fn run(root: &Path, o: Options, image: bool, list: bool) -> bool {
    if list {
        inventory::list(o, image);
        return true;
    }
    let (ctx, log) = match crate::run::start_logs(root) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("{}", e);
            return false;
        }
    };
    println!("complete logs: {}", log.display());
    let t = Instant::now();
    let mut results: Vec<Step> = Vec::new();
    for c in inventory::CHECKS {
        if !inventory::selected(c, o, image) {
            println!("not-selected {} ({:?})", c.id, c.selection);
            continue;
        }
        let missing: Vec<_> = c.after.iter().filter(|id| !results.iter().any(|s| s.name == **id && s.ok)).collect();
        results.push(report::step(c.id, || {
            if !missing.is_empty() {
                return Err(vec![format!("missing prerequisite results: {:?}", missing)]);
            }
            (c.run)(&ctx, root)
        }));
    }
    let tier = if image { "image" } else { "default" };
    let ok = report::summary(&results, t.elapsed().as_secs_f64(), o.fast || !o.coverage, tier);
    if let Err(e) = std::fs::write(log.join("steps.log"), format!("{results:#?}\nexit={}\n", u8::from(!ok))) {
        eprintln!("cannot retain step results: {}", e);
        return false;
    }
    ok
}
