//! The crates' tests, in debug and release, with full counts.

use crate::run;
use std::path::Path;

/// Summed `test result` lines of one cargo test run.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub passed: u64,
    pub failed: u64,
    pub ignored: u64,
}

impl std::fmt::Display for Counts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} passed, {} failed, {} ignored", self.passed, self.failed, self.ignored)
    }
}

/// The number before `word` in `line`: 12 for ("ok. 12 passed;", "passed").
fn before(line: &str, word: &str) -> u64 {
    let words: Vec<&str> = line.split_whitespace().collect();
    words.windows(2).find(|w| w[1].trim_end_matches([';', ',']) == word).and_then(|w| w[0].parse().ok()).unwrap_or(0)
}

/// Sums every `test result:` line of `out`.
pub fn counts(out: &str) -> Counts {
    out.lines().filter(|l| l.starts_with("test result:")).fold(Counts::default(), |c, l| Counts {
        passed: c.passed + before(l, "passed"),
        failed: c.failed + before(l, "failed"),
        ignored: c.ignored + before(l, "ignored"),
    })
}

/// Runs the tests of the crate in `root/krate`; `release` adds `--release`. Err with the end of the output when a
/// test fails or the build breaks.
pub fn run(ctx: &crate::run::Context, root: &Path, krate: &str, release: bool) -> Result<Counts, String> {
    // the committed Cargo.lock, never a rewritten one
    let mut args = vec!["test", "--locked"];
    if release {
        args.push("--release");
    }
    let out = run::capture(ctx, &root.join(krate), "cargo", &args)?;
    let c = counts(&out.stdout);
    if !out.ok || c.failed > 0 {
        return Err(format!("{}\n{}", run::tail(&out.stdout, 30), run::tail(&out.stderr, 20)));
    }
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sums_every_result_line() {
        let out = "running 3 tests\n\
                   test result: ok. 339 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 1s\n\
                   test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured\n";
        assert_eq!(counts(out), Counts { passed: 343, failed: 1, ignored: 2 });
        assert_eq!(counts("nothing"), Counts::default());
    }
}
