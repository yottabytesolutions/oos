// SPDX-License-Identifier: GPL-2.0-or-later
//! Host register plans against a reviewed baseline (ADR 0008).

use crate::run::{self, Context};
use crate::steps::{host, Outcome};
use std::path::Path;

#[cfg(test)]
mod tests;

const BASELINE: &str = "xtask/data/plan-matrix.txt";
const UPDATE: &str =
    "review the diff; for an intended change run cargo xtask plan-matrix --update and include the baseline in the PR";

fn cases() -> Vec<Vec<&'static str>> {
    include_str!("../../tools/plan-matrix-cases.txt")
        .lines()
        .map(|line| line.split_ascii_whitespace().collect())
        .collect()
}

fn generate(mut plan: impl FnMut(&[&str]) -> Result<run::Output, String>) -> Result<String, String> {
    let mut matrix = String::new();
    for case in cases() {
        let label = case.join(" ");
        let args: Vec<_> = std::iter::once("plan").chain(case.iter().copied()).collect();
        let out = plan(&args)?;
        if !out.ok {
            return Err(format!("plan {label} failed:\n{}{}", out.stdout, out.stderr));
        }
        matrix.push_str(&format!("### {label}\n{}{}", out.stdout, out.stderr));
    }
    Ok(matrix)
}

fn matrix(ctx: &Context, root: &Path) -> Result<String, Vec<String>> {
    // The CLI always searches these locations in addition to OOS_CAL_DIR.
    for path in ["/oos/cal", "/oos/limits.conf", "/etc/oos/limits.conf"] {
        if Path::new(path).exists() {
            return Err(vec![format!("{path}: local OOS defaults present; run plan-matrix on a host without them")]);
        }
    }
    host::build_fpgad_binary(ctx, root, "oos-fpgad")?;
    let target = host::target(ctx, root)?;
    let cal = target.join("plan-matrix-cal");
    host::command(ctx, root, "python3", &["-I", "tools/generate-test-cal.py", "--output", &cal.to_string_lossy()])?;
    let binary = target.join("debug/oos-fpgad");
    generate(|args| run::capture_env(ctx, root, &binary.to_string_lossy(), args, &[("OOS_CAL_DIR", cal.as_os_str())]))
        .map_err(|e| vec![e])
}

fn compare(ctx: &Context, root: &Path, actual: &str, update: bool) -> Outcome {
    let path = root.join(BASELINE);
    if update {
        std::fs::write(&path, actual).map_err(|e| vec![format!("{BASELINE}: {e}")])?;
        return Ok(vec![format!("updated {BASELINE}; review and explain the register changes in the PR")]);
    }
    let expected = std::fs::read_to_string(&path).map_err(|e| vec![format!("{BASELINE}: {e}"), UPDATE.into()])?;
    if expected == actual {
        let count = cases().len();
        return Ok(vec![format!("{count} plans match {BASELINE} (host defaults, synthetic calibration)")]);
    }
    let generated = host::target(ctx, root)?.join("plan-matrix.txt");
    std::fs::write(&generated, actual).map_err(|e| vec![e.to_string()])?;
    let diff = run::capture(
        ctx,
        root,
        "diff",
        &["-u", "-L", BASELINE, "-L", "generated plan matrix", &path.to_string_lossy(), &generated.to_string_lossy()],
    )
    .map_err(|e| vec![e, UPDATE.into()])?;
    Err(vec![format!("{BASELINE} differs:\n{}{}", diff.stdout, diff.stderr), UPDATE.into()])
}

pub(crate) fn check(ctx: &Context, root: &Path) -> Outcome {
    compare(ctx, root, &matrix(ctx, root)?, false)
}

pub(crate) fn run(root: &Path, update: bool) -> bool {
    let (ctx, logs) = match run::start_logs(root) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("{e}");
            return false;
        }
    };
    println!("complete logs: {}", logs.display());
    crate::report::step("plan-matrix", || compare(&ctx, root, &matrix(&ctx, root)?, update)).ok
}
