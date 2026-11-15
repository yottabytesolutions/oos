//! `cargo xtask check`: the gate.

use crate::report::{self, Step};
use crate::steps;
use std::path::Path;
use std::time::Instant;

/// What the gate runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// the partial subset for pre-commit: formatting, syntax rules, architecture and agent rules
    pub fast: bool,
    pub coverage: bool,
}

type GateStep = (&'static str, fn(&Path) -> steps::Outcome);

/// The ordered steps shared by fast and full dispatch.
pub(crate) fn plan(o: Options) -> Vec<GateStep> {
    let mut s: Vec<GateStep> = vec![
        ("fmt", steps::fmt),
        ("size limits", steps::size),
        ("architecture", steps::arch),
        ("mutable statics", steps::statics),
        ("unsafe", steps::unsafety),
        ("disallowed-method exceptions", steps::allows),
        ("known failures", steps::known),
        ("agent rules", crate::agent_rules::check),
    ];
    if !o.fast {
        s.push(("clippy", steps::clippy));
        s.push(("tests (debug)", |root| steps::tests(root, false)));
        s.push(("tests (release)", |root| steps::tests(root, true)));
        s.push(("perf guards", steps::perf));
        if o.coverage {
            s.push(("coverage", steps::coverage));
        }
    }
    s
}

/// Runs the gate; true when every step passed.
pub fn run(root: &Path, o: Options) -> bool {
    let t = Instant::now();
    let s: Vec<Step> = plan(o).into_iter().map(|(name, run)| report::step(name, || run(root))).collect();
    report::summary(&s, t.elapsed().as_secs_f64(), o.fast || !o.coverage)
}
