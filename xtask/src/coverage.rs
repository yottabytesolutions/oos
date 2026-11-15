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
pub fn installed() -> bool {
    run::available("cargo", &["llvm-cov", "--version"])
}

/// Line coverage of the crate in `root/krate`, in tenths of a percent. Runs the tests once more, instrumented.
pub fn measure(root: &Path, krate: &str) -> Result<u64, String> {
    let target = run::capture(root, "python3", &["tools/output.py", "target"])?;
    if !target.ok {
        return Err(target.stderr);
    }
    let dir = coverage_dir(Path::new(target.stdout.trim()));
    let out = run::capture_env(
        &root.join(krate),
        "cargo",
        &["llvm-cov", "--locked", "--json", "--summary-only"],
        &[("CARGO_TARGET_DIR", dir.as_os_str())],
    )?;
    if !out.ok {
        return Err(run::tail(&out.stderr, 20));
    }
    tenths(&out.stdout)
}

fn coverage_dir(target: &Path) -> std::path::PathBuf {
    target.join("xtask-cov")
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
        assert_eq!(coverage_dir(target), target.join("xtask-cov"));
    }
}
