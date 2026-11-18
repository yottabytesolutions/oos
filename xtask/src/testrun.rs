// SPDX-License-Identifier: GPL-2.0-or-later
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

/// The counts of a cargo-nextest `Summary` line: skipped tests count as ignored, timed-out ones as failed. None
/// without that line.
pub fn nextest_counts(out: &str) -> Option<Counts> {
    let line = out.lines().rev().find(|l| l.trim_start().starts_with("Summary ["))?;
    let tail = line.split_once(" run: ").map_or("", |(_, t)| t);
    Some(Counts {
        passed: before(tail, "passed"),
        failed: before(tail, "failed") + before(tail, "timed"),
        ignored: before(tail, "skipped"),
    })
}

/// The nextest result lines of tests that ran at least `SLOW_SECS`, so that the job log names them.
pub fn slow(out: &str) -> Vec<String> {
    out.lines()
        .map(str::trim)
        .filter(|l| {
            l.split_once('[')
                .and_then(|(status, rest)| (!matches!(status.trim(), "" | "Summary" | "SLOW")).then_some(rest))
                .and_then(|rest| rest.split_once("s]"))
                .and_then(|(secs, _)| secs.trim().parse::<f64>().ok())
                .is_some_and(|secs| secs >= SLOW_SECS)
        })
        .map(str::to_string)
        .collect()
}

/// Matches `slow-timeout` in `.config/nextest.toml`.
const SLOW_SECS: f64 = 60.0;

/// The Cargo profile of the release tests (root Cargo.toml): the release optimisation without a fat-LTO link per test
/// binary. HAL timing guards share this profile; UI guards keep the shipping `release` profile.
const RELEASE_TESTS: &str = "test-release";

fn golden_shard(shard: Option<(u32, u32)>) -> bool {
    shard.is_some_and(|(k, n)| n > 1 && k == n)
}

fn layout_shard(shard: Option<(u32, u32)>) -> bool {
    shard.is_some_and(|(k, n)| n > 2 && k == n - 1)
}

const LAYOUT_TEST: &str = "test(=ui::lint::second_pane_fits_with_everything_else)";

/// The cargo-nextest options for one crate, after `nextest run` or `llvm-cov nextest`. cargo-nextest runs every test
/// in its own process and the tests of all binaries in parallel; the doctests it cannot run go through `cargo test`.
pub fn nextest_args(root: &Path, release: bool, shard: Option<(u32, u32)>) -> Result<Vec<String>, String> {
    // Plain text: CI sets CARGO_TERM_COLOR=always, and colour codes hide the summary from the parser. Each crate is
    // its own Cargo workspace, so nextest would look for its configuration in the crate: name the repository's.
    let config = root.join(".config/nextest.toml");
    let config = config.to_str().ok_or("the repository path is not UTF-8")?;
    let mut args: Vec<String> =
        ["--locked", "--no-fail-fast", "--no-tests=pass", "--color", "never", "--config-file", config]
            .map(String::from)
            .into();
    if release {
        args.extend(["--cargo-profile".to_string(), RELEASE_TESTS.to_string()]);
    }
    if golden_shard(shard) {
        args.extend(["--test".to_string(), "golden".to_string()]);
    } else if layout_shard(shard) {
        args.extend(["--bin", "oos-ui", "-E", LAYOUT_TEST].map(String::from));
    } else if let Some((k, n)) = shard.filter(|(_, n)| *n > 1) {
        let filter = if n > 2 {
            format!("not (binary_id(=oos-ui::golden) or {LAYOUT_TEST})")
        } else {
            "not binary_id(=oos-ui::golden)".to_string()
        };
        args.extend([
            "-E".to_string(),
            filter,
            "--partition".to_string(),
            format!("hash:{k}/{}", n - 1 - u32::from(n > 2)),
        ]);
    }
    Ok(args)
}

fn fail(out: &run::Output) -> String {
    format!("{}\n{}", run::tail(&out.stdout, 30), run::tail(&out.stderr, 20))
}

/// Runs the tests of the crate in `root/krate`, with the release-tests profile when `release`. Ok with the counts and
/// the tests that ran 60 s or longer. Err with the end of the output when a test fails or the build breaks.
pub fn run(
    ctx: &crate::run::Context,
    root: &Path,
    krate: &str,
    release: bool,
) -> Result<(Counts, Vec<String>), String> {
    let shard = run::partition("OOS_TEST_PARTITION", std::env::var("OOS_TEST_PARTITION").ok().as_deref())?;
    if (golden_shard(shard) || layout_shard(shard)) && krate != "ui" {
        return Ok((Counts::default(), Vec::new()));
    }
    let dir = root.join(krate);
    let args = nextest_args(root, release, shard)?;
    let args: Vec<&str> = ["nextest", "run"].into_iter().chain(args.iter().map(String::as_str)).collect();
    let out = run::capture(ctx, &dir, "cargo", &args)?;
    let text = format!("{}\n{}", out.stdout, out.stderr);
    let Some(mut c) = nextest_counts(&text) else {
        return Err(format!("no nextest summary\n{}", fail(&out)));
    };
    if !out.ok || c.failed > 0 {
        return Err(fail(&out));
    }
    // The doctests run once, in the first shard.
    if dir.join("src/lib.rs").is_file() && shard.is_none_or(|(k, _)| k == 1) {
        let profile: &[&str] = if release { &["--profile", RELEASE_TESTS] } else { &[] };
        let doc = run::capture(ctx, &dir, "cargo", &[&["test", "--locked", "--doc"], profile].concat())?;
        let d = counts(&doc.stdout);
        if !doc.ok || d.failed > 0 {
            return Err(fail(&doc));
        }
        c = Counts { passed: c.passed + d.passed, failed: 0, ignored: c.ignored + d.ignored };
    }
    Ok((c, slow(&text)))
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

    #[test]
    fn reads_the_nextest_summary() {
        let out = "    PASS [   0.004s] oos-core model::tests::x\n\
                   ------------\n\
                   \u{20}    Summary [   1.234s] 341 tests run: 339 passed, 2 failed, 5 skipped\n";
        assert_eq!(nextest_counts(out), Some(Counts { passed: 339, failed: 2, ignored: 5 }));
        let timeout = "     Summary [  60.000s] 3 tests run: 2 passed, 1 timed out, 0 skipped\n";
        assert_eq!(nextest_counts(timeout), Some(Counts { passed: 2, failed: 1, ignored: 0 }));
        let empty = "     Summary [   0.000s] 0 tests run: 0 passed, 0 skipped\n";
        assert_eq!(nextest_counts(empty), Some(Counts::default()));
        assert_eq!(nextest_counts("no summary"), None);
        let coloured = "\u{1b}[1;32m     Summary\u{1b}[0m [ 1.0s] 3 tests run: 3 passed, 0 skipped\n";
        assert_eq!(nextest_counts(coloured), None, "colour hides the summary: the step asks for plain text");
    }

    #[test]
    fn partition_args_keep_the_profile_and_two_shard_coverage() {
        let args = nextest_args(Path::new("/path/to/oos"), true, Some((1, 2))).unwrap();
        assert!(args.windows(2).any(|a| a == ["--cargo-profile", "test-release"]));
        assert_eq!(args[args.len() - 4..], ["-E", "not binary_id(=oos-ui::golden)", "--partition", "hash:1/1"]);
        let debug = nextest_args(Path::new("/path/to/oos"), false, None).unwrap();
        assert!(!debug.iter().any(|a| a == "--partition" || a == "--cargo-profile"));
    }

    #[test]
    fn layout_and_golden_have_disjoint_shards_while_unsharded_runs_keep_every_test() {
        for release in [false, true] {
            let root = Path::new("/path/to/oos");
            for k in 1..4 {
                let args = nextest_args(root, release, Some((k, 5))).unwrap();
                assert_eq!(
                    args[args.len() - 4..],
                    [
                        "-E",
                        &format!("not (binary_id(=oos-ui::golden) or {LAYOUT_TEST})"),
                        "--partition",
                        &format!("hash:{k}/3")
                    ]
                );
            }
            let layout = nextest_args(root, release, Some((4, 5))).unwrap();
            assert_eq!(layout[layout.len() - 4..], ["--bin", "oos-ui", "-E", LAYOUT_TEST]);
            assert!(!layout.iter().any(|a| a == "--partition"));
            let golden = nextest_args(root, release, Some((5, 5))).unwrap();
            assert_eq!(golden[golden.len() - 2..], ["--test", "golden"]);
            assert!(!golden.iter().any(|a| a == "--partition"));
            for shard in [None, Some((1, 1))] {
                let all = nextest_args(root, release, shard).unwrap();
                assert!(!all.iter().any(|a| a == "--partition" || a == "-E" || a == "--test" || a == "--bin"));
            }
            let two = nextest_args(root, release, Some((2, 2))).unwrap();
            assert_eq!(two[two.len() - 2..], ["--test", "golden"]);
        }
    }

    #[test]
    fn names_the_tests_that_ran_a_minute_or_longer() {
        let out = "        PASS [  26.171s] (455/456) oos-hal update::tests::short\n\
                   \u{20}       SLOW [> 60.000s] (─────────) oos-ui::golden compare_scripted_pictures\n\
                   \u{20}       PASS [  73.402s] (456/456) oos-ui::golden compare_scripted_pictures\n\
                   \u{20}    Summary [ 118.865s] 1465 tests run: 1465 passed (1 slow), 58 skipped\n";
        assert_eq!(slow(out), ["PASS [  73.402s] (456/456) oos-ui::golden compare_scripted_pictures"]);
        assert!(slow("no result lines").is_empty());
    }
}
