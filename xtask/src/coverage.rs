// SPDX-License-Identifier: GPL-2.0-or-later
//! Line coverage per crate with cargo-llvm-cov, against a floor that can only rise (`xtask/data/coverage.txt`, in
//! tenths of a percent).

use crate::run;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Report {
    data: Vec<Data>,
}

#[derive(Debug, Deserialize)]
struct Data {
    totals: Totals,
}

#[derive(Debug, Deserialize)]
struct Totals {
    lines: Lines,
}

#[derive(Debug, Deserialize)]
struct Lines {
    count: u64,
    covered: u64,
}

/// Covered lines in tenths of a percent, from the `--json --summary-only` report.
fn tenths(json: &str) -> Result<u64, String> {
    let r: Report = serde_json::from_str(json).map_err(|e| format!("cargo llvm-cov report: {}", e))?;
    let l = r.data.first().map(|d| &d.totals.lines).ok_or("cargo llvm-cov report: no data")?;
    Ok((l.covered * 1000).checked_div(l.count).unwrap_or(0))
}

/// Whether cargo-llvm-cov is installed.
pub fn installed(ctx: &crate::run::Context, root: &Path) -> bool {
    run::available(ctx, root, "cargo", &["llvm-cov", "--version"])
}

/// Line coverage of the crate in `root/krate`, in tenths of a percent. Runs the tests once more, instrumented, under
/// cargo-nextest.
pub fn measure(ctx: &crate::run::Context, root: &Path, krate: &str) -> Result<u64, String> {
    let target = run::capture(ctx, root, "python3", &["-I", "tools/output.py", "target"])?;
    if !target.ok {
        return Err(target.stderr);
    }
    let dir = coverage_dir(Path::new(target.stdout.trim()), krate);
    let args = crate::testrun::nextest_args(root, false, None)?;
    let args: Vec<&str> = ["llvm-cov", "nextest", "--json", "--summary-only"]
        .into_iter()
        .chain(args.iter().map(String::as_str))
        .collect();
    let cargo = |args: &[&str]| {
        run::capture_env(ctx, &root.join(krate), "cargo", args, &[("CARGO_TARGET_DIR", dir.as_os_str())])
    };
    let out = cargo(&args)?;
    finish(&out, || cargo(&["llvm-cov", "clean", "--locked", "--profraw-only"]))
}

/// Raw profiles are redundant once merged. Keep binaries and profdata for reports, and failed runs for diagnosis.
fn finish(out: &run::Output, clean: impl FnOnce() -> Result<run::Output, String>) -> Result<u64, String> {
    if !out.ok {
        return Err(run::tail(&out.stderr, 20));
    }
    let covered = tenths(&out.stdout)?;
    let cleaned = clean()?;
    if !cleaned.ok {
        return Err(format!("coverage profile cleanup failed: {}", cleaned.stderr));
    }
    Ok(covered)
}

/// A target directory per crate: cargo-llvm-cov empties its directory of the previous profiles, so the crates can
/// only be measured at the same time in directories of their own.
fn coverage_dir(target: &Path, krate: &str) -> std::path::PathBuf {
    target.join("xtask-cov").join(krate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_line_totals() {
        let j = r#"{"data":[{"totals":{"lines":{"count":9314,"covered":8844,"percent":94.9}}}],"type":"x"}"#;
        assert_eq!(tenths(j).unwrap(), 949);
        assert_eq!(tenths(r#"{"data":[{"totals":{"lines":{"count":0,"covered":0}}}]}"#).unwrap(), 0);
        assert!(tenths("{}").is_err());
    }

    #[test]
    fn coverage_follows_the_selected_target() {
        let target = Path::new("/path/to/worktree/build");
        assert_eq!(coverage_dir(target, "ui"), target.join("xtask-cov/ui"));
    }

    #[test]
    fn raw_profiles_are_cleaned_only_after_a_valid_report() {
        let report = |ok, stdout: &str| run::Output { ok, stdout: stdout.into(), stderr: "coverage failed".into() };
        let valid = report(true, r#"{"data":[{"totals":{"lines":{"count":10,"covered":9}}}]}"#);
        let cleaned = std::cell::Cell::new(false);
        assert_eq!(
            finish(&valid, || {
                cleaned.set(true);
                Ok(report(true, ""))
            }),
            Ok(900)
        );
        assert!(cleaned.get());
        for invalid in [report(false, ""), report(true, "malformed report")] {
            assert!(finish(&invalid, || panic!("keep failed coverage evidence")).is_err());
        }
        assert_eq!(finish(&valid, || Err("cannot start cleanup".into())), Err("cannot start cleanup".into()));
        assert!(finish(&valid, || Ok(report(false, ""))).unwrap_err().contains("cleanup failed"));
    }
}
