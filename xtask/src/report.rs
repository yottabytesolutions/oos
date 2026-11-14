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

/// The closing line; true when every step passed.
pub fn summary(steps: &[Step], total: f64) -> bool {
    let failed: Vec<&str> = steps.iter().filter(|s| !s.ok).map(|s| s.name.as_str()).collect();
    if failed.is_empty() {
        println!("gate passed: {} steps in {:.0} s", steps.len(), total);
    } else {
        println!("gate FAILED in {:.0} s: {}", total, failed.join(", "));
    }
    failed.is_empty()
}
