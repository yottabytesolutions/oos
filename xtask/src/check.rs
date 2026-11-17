//! `cargo xtask check`: execute the inventory and preserve incomplete evidence.
use crate::inventory;
#[cfg(test)]
use crate::inventory::Selection;
use crate::report::{self, Step};
use crate::steps;
use std::path::Path;
use std::sync::{Condvar, Mutex};
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
    let tier = if image { "image" } else { "default" };
    execute(root, |c| inventory::selected(c, o, image), o.fast || !o.coverage, tier)
}

/// An error naming every id that is not in the inventory.
pub fn known(ids: &[&str]) -> Result<(), String> {
    let unknown: Vec<&str> = ids.iter().copied().filter(|id| !inventory::CHECKS.iter().any(|c| c.id == *id)).collect();
    if unknown.is_empty() {
        return Ok(());
    }
    Err(format!("unknown step {:?}; `cargo xtask check --list` prints the ids", unknown))
}

/// `cargo xtask step`: the named steps in inventory order. Always a partial gate; prerequisites must be named too.
pub fn steps(root: &Path, ids: &[&str]) -> bool {
    execute(root, |c| ids.contains(&c.id), true, "default")
}

/// Runs the wanted checks: each one starts as soon as its prerequisites have passed, all at once, except the checks
/// that must run alone (`inventory::alone`), which follow one by one when the others have finished. The results keep
/// the inventory order.
fn execute(root: &Path, wanted: impl Fn(&inventory::Check) -> bool, partial: bool, tier: &str) -> bool {
    let (ctx, log) = match crate::run::start_logs(root) {
        Ok(path) => path,
        Err(e) => {
            eprintln!("{}", e);
            return false;
        }
    };
    println!("complete logs: {}", log.display());
    let t = Instant::now();
    let mut chosen = Vec::new();
    for c in inventory::CHECKS {
        if wanted(c) {
            chosen.push(c);
        } else {
            println!("not-selected {} ({:?})", c.id, c.selection);
        }
    }
    let ids: Vec<&str> = chosen.iter().map(|c| c.id).collect();
    let done: Mutex<Vec<Step>> = Mutex::new(Vec::new());
    let changed = Condvar::new();
    let run = |c: &inventory::Check| {
        let missing = wait_for(c, &ids, &done, &changed);
        let step = report::step(c.id, || {
            if !missing.is_empty() {
                return Err(vec![format!("missing prerequisite results: {:?}", missing)]);
            }
            (c.run)(&ctx, root)
        });
        lock(&done).push(step);
        changed.notify_all();
    };
    let (alone, together): (Vec<_>, Vec<_>) = chosen.iter().partition(|c| inventory::alone(c));
    std::thread::scope(|s| {
        for c in together {
            let run = &run;
            s.spawn(move || run(c));
        }
    });
    alone.into_iter().for_each(run);
    let mut results = done.into_inner().unwrap_or_else(std::sync::PoisonError::into_inner);
    results.sort_by_key(|s| chosen.iter().position(|c| c.id == s.name));
    let ok = report::summary(&results, t.elapsed().as_secs_f64(), partial, tier);
    if let Err(e) = std::fs::write(log.join("steps.log"), format!("{results:#?}\nexit={}\n", u8::from(!ok))) {
        eprintln!("cannot retain step results: {}", e);
        return false;
    }
    ok
}

fn lock(done: &Mutex<Vec<Step>>) -> std::sync::MutexGuard<'_, Vec<Step>> {
    done.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Waits until every prerequisite of `c` among the `chosen` checks has a result; returns the prerequisites that did
/// not pass. One that was not chosen never gets a result and counts as missing at once.
fn wait_for(c: &inventory::Check, chosen: &[&str], done: &Mutex<Vec<Step>>, changed: &Condvar) -> Vec<&'static str> {
    let mut steps = lock(done);
    loop {
        let result = |id: &str| steps.iter().find(|s| s.name == id).map(|s| s.ok);
        let pending = c.after.iter().any(|id| result(id).is_none() && chosen.contains(id));
        if !pending {
            return c.after.iter().copied().filter(|id| result(id) != Some(true)).collect();
        }
        steps = changed.wait(steps).unwrap_or_else(std::sync::PoisonError::into_inner);
    }
}
