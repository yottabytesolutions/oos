//! The gate's steps over every crate. Each returns its summary lines, or the failures.

use crate::ratchet::Ratchet;
use crate::repo::{self, CRATES};
use crate::{arch, clippy, coverage, fmt, size, target, testrun};
use std::path::Path;

type Outcome = Result<Vec<String>, Vec<String>>;

/// The ratchet file `name` in the data directory.
pub fn ratchet_path(root: &Path, name: &str) -> std::path::PathBuf {
    repo::data(root).join(name)
}

/// Entries of `r` that belong to `krate`: the second word is the crate or a path in it.
pub fn of_crate(r: &Ratchet, krate: &str) -> Ratchet {
    let prefix = format!("{}/", krate);
    Ratchet(
        r.0.iter()
            .filter(|(k, _)| {
                k.split(' ').nth(1).is_some_and(|w| w == krate || w.starts_with(&prefix))
                    || k.starts_with(&format!("{} ", krate))
            })
            .map(|(k, v)| (k.clone(), *v))
            .collect(),
    )
}

fn outcome(ok: Vec<String>, bad: Vec<String>) -> Outcome {
    if bad.is_empty() {
        Ok(ok)
    } else {
        Err(bad)
    }
}

pub fn fmt(root: &Path) -> Outcome {
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for c in CRATES {
        match fmt::diff_lines(root, c.dir) {
            Ok(0) => ok.push(format!("{}: formatted", c.dir)),
            Ok(n) => bad.push(format!("{}: {} lines differ, run cargo fmt in {}", c.dir, n, c.dir)),
            Err(e) => bad.push(format!("{}: {}", c.dir, e)),
        }
    }
    outcome(ok, bad)
}

/// Compares `now` of every crate with the ratchet file; crates without a ratchet allow nothing.
fn ratcheted(root: &Path, file: &str, now: &[(&'static str, bool, Ratchet)]) -> Outcome {
    let allowed = Ratchet::load(&ratchet_path(root, file)).map_err(|e| vec![e])?;
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for (krate, has_ratchet, n) in now {
        let a = if *has_ratchet { of_crate(&allowed, krate) } else { Ratchet::default() };
        let v = a.compare(n);
        bad.extend(v.failures().into_iter().map(|f| format!("{}: {}", krate, f)));
        let total: u64 = n.0.values().sum();
        let shrunk = if v.shrunk.is_empty() {
            String::new()
        } else {
            format!(", {} improved (cargo xtask ratchet)", v.shrunk.len())
        };
        ok.push(format!("{}: {} entries, {} total{}", krate, n.0.len(), total, shrunk));
    }
    outcome(ok, bad)
}

pub fn clippy(root: &Path) -> Outcome {
    let mut now = Vec::new();
    let mut errors = Vec::new();
    for c in CRATES {
        match clippy::run(root, c.dir) {
            Ok(f) => {
                errors.extend(f.errors.into_iter().map(|e| format!("{}: {}", c.dir, e)));
                now.push((c.dir, c.ratchet, f.counts));
            }
            Err(e) => errors.push(format!("{}: {}", c.dir, e)),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    ratcheted(root, "clippy.txt", &now)
}

pub fn size(root: &Path) -> Outcome {
    let mut now = Vec::new();
    for c in CRATES {
        now.push((c.dir, c.ratchet, size::measure(root, c.dir).map_err(|e| vec![e])?.sizes));
    }
    ratcheted(root, "size.txt", &now)
}

/// Statics with interior mutability outside the metrics allowlist, per crate.
pub fn statics_now(root: &Path, krate: &str, metrics: &std::collections::BTreeSet<String>) -> Result<Ratchet, String> {
    let mut s = size::measure(root, krate)?.statics;
    s.0.retain(|k, _| !metrics.contains(k));
    Ok(s)
}

/// Mutable statics: only metrics (allowlist in architecture.txt) and the listed old ones (statics.txt).
pub fn statics(root: &Path) -> Outcome {
    let rules = arch::load(root).map_err(|e| vec![e])?;
    let mut now = Vec::new();
    let mut all = std::collections::BTreeSet::new();
    for c in CRATES {
        let mut m = size::measure(root, c.dir).map_err(|e| vec![e])?.statics;
        all.extend(m.0.keys().cloned());
        m.0.retain(|k, _| !rules.statics.contains(k));
        now.push((c.dir, c.ratchet, m));
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

pub fn arch(root: &Path) -> Outcome {
    let v = arch::check(root).map_err(|e| vec![e])?;
    outcome(vec!["crate and module rules hold".into()], v)
}

pub fn tests(root: &Path, release: bool) -> Outcome {
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for c in CRATES {
        match testrun::run(root, c.dir, release) {
            Ok(n) => ok.push(format!("{}: {}", c.dir, n)),
            Err(e) => bad.push(format!("{}: {}", c.dir, e)),
        }
    }
    outcome(ok, bad)
}

pub fn target(root: &Path) -> Outcome {
    if !target::installed() {
        return Err(vec![format!("rustup target add {}", repo::TARGET)]);
    }
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for c in CRATES.iter().filter(|c| c.target) {
        match target::check(root, c.dir) {
            Ok(()) => ok.push(format!("{}: checks for {}", c.dir, repo::TARGET)),
            Err(e) => bad.push(format!("{}: {}", c.dir, e)),
        }
    }
    outcome(ok, bad)
}

/// The fork's performance guards (`tools/perf-guard.sh`): its exit code decides, its output only explains.
pub fn perf(root: &Path) -> Outcome {
    let out = crate::run::capture(root, "sh", &["tools/perf-guard.sh"]).map_err(|e| vec![e])?;
    let text = format!("{}{}", out.stdout, out.stderr);
    if out.ok {
        Ok(vec![text.lines().last().unwrap_or("passed").to_string()])
    } else {
        Err(vec![crate::run::tail(&text, 30)])
    }
}

/// Current coverage of every crate, in tenths of a percent.
pub fn coverage_now(root: &Path) -> Result<Ratchet, Vec<String>> {
    let mut now = Ratchet::default();
    for c in CRATES {
        now.0.insert(c.dir.to_string(), coverage::measure(root, c.dir).map_err(|e| vec![format!("{}: {}", c.dir, e)])?);
    }
    Ok(now)
}

pub fn coverage(root: &Path) -> Outcome {
    if !coverage::installed() {
        return Err(vec!["cargo install cargo-llvm-cov (or run with --no-coverage)".into()]);
    }
    let floor = Ratchet::load(&ratchet_path(root, "coverage.txt")).map_err(|e| vec![e])?;
    let now = coverage_now(root)?;
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for (k, &n) in &now.0 {
        let f = floor.0.get(k).copied().unwrap_or(0);
        let line = format!("{}: {}.{}% (floor {}.{}%)", k, n / 10, n % 10, f / 10, f % 10);
        if n < f {
            bad.push(line);
        } else {
            ok.push(line);
        }
    }
    outcome(ok, bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_split_by_crate() {
        let r = Ratchet::parse("3 file ui/src/a.rs\n2 impls ui Ui\n9 ui ui/src/a.rs clippy::x\n4 file hal/src/b.rs\n")
            .unwrap();
        assert_eq!(of_crate(&r, "ui").0.len(), 3);
        assert_eq!(of_crate(&r, "hal").0.len(), 1);
        assert!(of_crate(&r, "u").0.is_empty());
    }
}
