//! The gate's steps over every crate. Each returns its summary lines, or the failures.

mod code;

pub use code::{measured, size, statics, unsafe_now, unsafety};

use crate::ratchet::Ratchet;
use crate::repo::{self, CRATES};
use crate::{arch, base, clippy, coverage, fmt, target, testrun, workspace};
use std::path::Path;

pub(crate) type Outcome = Result<Vec<String>, Vec<String>>;

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

pub(crate) fn outcome(ok: Vec<String>, bad: Vec<String>) -> Outcome {
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

/// The ratchet file `file`, checked against the base: it may not list more or larger entries than at the merge base.
fn allowed(root: &Path, file: &str) -> Result<Ratchet, Vec<String>> {
    let allowed = Ratchet::load(&ratchet_path(root, file)).map_err(|e| vec![e])?;
    let rel = format!("xtask/data/{}", file);
    if let Some(t) = base::text(root, &rel).map_err(|e| vec![e])? {
        let widened =
            base::widened(&Ratchet::parse(&t).map_err(|e| vec![format!("{} at the base: {}", rel, e)])?, &allowed);
        if !widened.is_empty() {
            return Err(widened.into_iter().map(|w| format!("{}: {}", rel, w)).collect());
        }
    }
    Ok(allowed)
}

/// Compares `now` of every crate with the ratchet file; crates without a ratchet allow nothing.
pub(crate) fn ratcheted(root: &Path, file: &str, now: &[(&'static str, bool, Ratchet)]) -> Outcome {
    let allowed = allowed(root, file)?;
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

/// Clippy for the host and, for the scope's crates, for aarch64-unknown-linux-musl: this also checks that the scope
/// build compiles, with the image build's target flags.
pub fn clippy(root: &Path) -> Outcome {
    if !target::installed() {
        return Err(vec![format!("rustup target add {}", repo::TARGET)]);
    }
    let mut now = Vec::new();
    let mut errors = Vec::new();
    for c in CRATES {
        match clippy::run(root, c.dir, c.target) {
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

pub fn arch(root: &Path) -> Outcome {
    let meta = workspace::load(root).map_err(|e| vec![e])?;
    let v = arch::check(root, &meta).map_err(|e| vec![e])?;
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
    if let Some(t) = base::text(root, "xtask/data/coverage.txt").map_err(|e| vec![e])? {
        let lowered = base::lowered(&Ratchet::parse(&t).map_err(|e| vec![e])?, &floor);
        if !lowered.is_empty() {
            return Err(lowered);
        }
    }
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
