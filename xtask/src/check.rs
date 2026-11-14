//! `cargo xtask check`: the gate.

use crate::report::{self, Step};
use crate::steps;
use std::path::Path;
use std::time::Instant;

/// What the gate runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// the seconds-long subset for pre-commit: formatting, size limits, architecture
    pub fast: bool,
    pub coverage: bool,
}

/// Runs the gate; true when every step passed.
pub fn run(root: &Path, o: Options) -> bool {
    let t = Instant::now();
    let mut s: Vec<Step> = vec![
        report::step("fmt", || steps::fmt(root)),
        report::step("size limits", || steps::size(root)),
        report::step("architecture", || steps::arch(root)),
        report::step("mutable statics", || steps::statics(root)),
    ];
    if !o.fast {
        s.push(report::step("clippy", || steps::clippy(root)));
        s.push(report::step("tests (debug)", || steps::tests(root, false)));
        s.push(report::step("tests (release)", || steps::tests(root, true)));
        s.push(report::step("scope target", || steps::target(root)));
        s.push(report::step("perf guards", || steps::perf(root)));
        if o.coverage {
            s.push(report::step("coverage", || steps::coverage(root)));
        }
    }
    report::summary(&s, t.elapsed().as_secs_f64())
}
