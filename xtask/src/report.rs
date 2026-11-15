//! The gate's report: one line per step, its time, and the details of a failure.

use std::time::Instant;

/// The outcome of one step.
#[derive(Debug)]
pub struct Step {
    pub name: String,
    pub secs: f64,
    pub ok: bool,
    /// summary on success, the failures otherwise
    pub lines: Vec<String>,
}

/// Runs `f` as the step `name`, timed, and prints its line at once.
pub fn step(name: &str, f: impl FnOnce() -> Result<Vec<String>, Vec<String>>) -> Step {
    let t = Instant::now();
    let (ok, lines) = match f() {
        Ok(l) => (true, l),
        Err(l) => (false, l),
    };
    let s = Step { name: name.to_string(), secs: t.elapsed().as_secs_f64(), ok, lines };
    print(&s);
    s
}

fn print(s: &Step) {
    println!("{:<4} {} ({:.1} s)", if s.ok { "ok" } else { "FAIL" }, s.name, s.secs);
    for l in &s.lines {
        for part in l.lines() {
            println!("       {}", part);
        }
    }
}

/// The closing line; true when every step passed. A subset (`partial`) never says the gate passed: the done rule is
/// the full gate.
pub fn summary(steps: &[Step], total: f64, partial: bool, tier: &str) -> bool {
    println!("{}", closing(steps, total, partial, tier));
    steps.iter().all(|s| s.ok)
}

fn closing(steps: &[Step], total: f64, partial: bool, tier: &str) -> String {
    let failed: Vec<&str> = steps.iter().filter(|s| !s.ok).map(|s| s.name.as_str()).collect();
    match (failed.is_empty(), partial) {
        (true, false) => format!("gate passed ({tier}): {} steps in {:.0} s", steps.len(), total),
        (true, true) => format!("partial gate passed (not done): {} steps in {:.0} s", steps.len(), total),
        (false, _) => format!("gate FAILED in {:.0} s: {}", total, failed.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The hole: --fast and --no-coverage printed "gate passed" like the full gate.
    #[test]
    fn a_partial_run_never_says_the_gate_passed() {
        let s = |ok| Step { name: "fmt".into(), secs: 0.0, ok, lines: vec![] };
        for tier in ["default", "image"] {
            assert_eq!(closing(&[s(true)], 3.0, false, tier), format!("gate passed ({tier}): 1 steps in 3 s"));
            assert_eq!(closing(&[s(true)], 3.0, true, tier), "partial gate passed (not done): 1 steps in 3 s");
            assert_eq!(closing(&[s(false)], 3.0, true, tier), "gate FAILED in 3 s: fmt");
            assert_eq!(closing(&[s(false)], 3.0, false, tier), "gate FAILED in 3 s: fmt");
        }
    }
}
