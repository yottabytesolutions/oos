// SPDX-License-Identifier: GPL-2.0-or-later
//! The gate's steps over every crate. Each returns its summary lines, or the failures.

mod code;
pub(crate) mod decoders;
pub(crate) mod host;
pub(crate) mod image;
pub(crate) mod python;
mod ratchets;
pub(crate) mod scripts;

pub use code::{allows, known, measured, size, statics, unsafe_now, unsafety};
pub use ratchets::load_moves;

use crate::store::Kind;
pub(crate) use ratchets::ratcheted;

use crate::ratchet::Ratchet;
use crate::repo::{self, CRATES};
use crate::{arch, base, clippy, coverage, fmt, target, testrun, workspace};
use std::path::Path;

pub(crate) type Outcome = Result<Vec<String>, Vec<String>>;

/// The ratchet file `name` in the data directory.
pub fn ratchet_path(root: &Path, name: &str) -> std::path::PathBuf {
    repo::data(root).join(name)
}

pub(crate) fn outcome(ok: Vec<String>, bad: Vec<String>) -> Outcome {
    if bad.is_empty() {
        Ok(ok)
    } else {
        Err(bad)
    }
}

pub fn fmt(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for (c, diff) in CRATES.iter().zip(repo::each(|c| fmt::diff_lines(ctx, root, c.dir))) {
        match diff {
            Ok(0) => ok.push(format!("{}: formatted", c.dir)),
            Ok(n) => bad.push(format!("{}: {} lines differ, run cargo fmt in {}", c.dir, n, c.dir)),
            Err(e) => bad.push(format!("{}: {}", c.dir, e)),
        }
    }
    outcome(ok, bad)
}

/// Clippy for the host and, for the scope's crates, for aarch64-unknown-linux-musl: this also checks that the scope
/// build compiles, with the image build's target flags.
pub fn clippy(ctx: &crate::run::Context, root: &Path) -> Outcome {
    if !target::installed(ctx, root) {
        return Err(vec![format!("rustup target add {}", repo::TARGET)]);
    }
    let mut now = Vec::new();
    let mut errors = Vec::new();
    for c in CRATES {
        match clippy::run(ctx, root, c.dir, c.target) {
            Ok(f) => {
                errors.extend(f.errors.into_iter().map(|e| format!("{}: {}", c.dir, e)));
                now.push((c.dir, f.counts));
            }
            Err(e) => errors.push(format!("{}: {}", c.dir, e)),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    ratcheted(ctx, root, Kind::Clippy, &now)
}

pub fn arch(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let meta = workspace::load(ctx, root).map_err(|e| vec![e])?;
    let v = arch::check(root, &meta).map_err(|e| vec![e])?;
    outcome(vec!["crate and module rules hold".into()], v)
}

pub fn tests(ctx: &crate::run::Context, root: &Path, release: bool) -> Outcome {
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for (c, run) in CRATES.iter().zip(repo::each(|c| testrun::run(ctx, root, c.dir, release))) {
        match run {
            Ok((n, slow)) => {
                ok.push(format!("{}: {}", c.dir, n));
                ok.extend(slow.into_iter().map(|line| format!("{}: slow: {}", c.dir, line)));
            }
            Err(e) => bad.push(format!("{}: {}", c.dir, e)),
        }
    }
    outcome(ok, bad)
}

/// The fork's performance guards (`tools/perf-guard.sh`): its exit code decides, its output only explains.
pub fn perf(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let out = crate::run::capture(ctx, root, "sh", &["tools/perf-guard.sh"]).map_err(|e| vec![e])?;
    let text = format!("{}{}", out.stdout, out.stderr);
    if out.ok {
        Ok(vec![text.lines().last().unwrap_or("passed").to_string()])
    } else {
        Err(vec![crate::run::tail(&text, 30)])
    }
}

/// Current coverage of every crate, in tenths of a percent.
pub fn coverage_now(ctx: &crate::run::Context, root: &Path) -> Result<Ratchet, Vec<String>> {
    let mut now = Ratchet::default();
    for (c, measured) in CRATES.iter().zip(repo::each(|c| coverage::measure(ctx, root, c.dir))) {
        now.0.insert(c.dir.to_string(), measured.map_err(|e| vec![format!("{}: {}", c.dir, e)])?);
    }
    Ok(now)
}

/// A branch may fall this far below a coverage floor (tenths of a percent): measurements move a little between
/// runs, and the integrator raises the floors after the merge (`cargo xtask ratchet --coverage`).
pub const MARGIN: u64 = 5;

/// Coverage `n` fails the floor `f` only when it is more than the margin below it.
fn below(n: u64, f: u64) -> bool {
    n + MARGIN < f
}

pub fn coverage(ctx: &crate::run::Context, root: &Path) -> Outcome {
    if !coverage::installed(ctx, root) {
        return Err(vec!["cargo install cargo-llvm-cov (or run with --no-coverage)".into()]);
    }
    let floor = Ratchet::load(&ratchet_path(root, "coverage.txt")).map_err(|e| vec![e])?;
    if let Some(t) = base::text(ctx, root, "xtask/data/coverage.txt").map_err(|e| vec![e])? {
        let lowered = base::lowered(&Ratchet::parse(&t).map_err(|e| vec![e])?, &floor);
        if !lowered.is_empty() {
            return Err(lowered);
        }
    }
    let now = coverage_now(ctx, root)?;
    let (mut ok, mut bad) = (Vec::new(), Vec::new());
    for (k, &n) in &now.0 {
        let f = floor.0.get(k).copied().unwrap_or(0);
        let line = format!(
            "{}: {}.{}% (floor {}.{}%, margin {}.{})",
            k,
            n / 10,
            n % 10,
            f / 10,
            f % 10,
            MARGIN / 10,
            MARGIN % 10
        );
        if below(n, f) {
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
    fn coverage_may_fall_half_a_point() {
        assert!(!below(900, 900) && !below(895, 900) && below(894, 900) && !below(950, 900));
    }
}
