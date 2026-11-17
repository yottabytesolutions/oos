// SPDX-License-Identifier: GPL-2.0-or-later
//! Isolated unittest discovery, execution counts and skip evidence for every registered root.
use super::Outcome;
use crate::run::{self, Context};
use std::path::Path;

#[cfg(test)]
mod tests;

const ROOTS: &[&str] = &["tools/tests", "tools/privacy", "ui/tools"];
const COUNT: &str =
    "import sys, unittest; print(unittest.TestLoader().discover(sys.argv[1], pattern='test_*.py').countTestCases())";

fn validate_roots(ctx: &Context, root: &Path) -> Result<(), String> {
    let paths = crate::agent_rules::inventory(ctx, root)?;
    if paths.is_empty() {
        return Err("empty Git discovery; restore tracked scripts".into());
    }
    let missing: Vec<_> = paths
        .iter()
        .filter(|name| {
            let path = Path::new(name);
            path.file_name().is_some_and(|n| n.to_string_lossy().starts_with("test_"))
                && path.extension().is_some_and(|ext| ext == "py")
                && root.join(path).is_file()
                && !ROOTS.iter().any(|directory| path.starts_with(directory))
        })
        .collect();
    if !missing.is_empty() {
        return Err(format!("Python tests outside discovered roots: {missing:?}; register their runner"));
    }
    Ok(())
}

fn summary(text: &str, expected: u64) -> Result<Vec<String>, String> {
    let counts: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("Ran "))
        .filter_map(|tail| tail.split_once(" test in ").or_else(|| tail.split_once(" tests in ")))
        .filter_map(|(count, _)| count.parse::<u64>().ok())
        .collect();
    if expected == 0 || counts != [expected] {
        return Err("test selection differs from executed test count".into());
    }
    let skip_count = text
        .split_once("skipped=")
        .map(|(_, tail)| tail.chars().take_while(char::is_ascii_digit).collect::<String>().parse::<u64>())
        .transpose()
        .map_err(|e| e.to_string())?;
    let skipped = skip_count.unwrap_or(0);
    let reasons: Vec<_> = text
        .lines()
        .filter_map(|l| l.split_once("... skipped ").map(|(_, r)| r))
        .filter(|reason| !reason.is_empty())
        .collect();
    let mut lines = vec![format!("python-tests: {expected} executed; {skipped} skipped")];
    lines.extend(reasons.iter().map(|reason| format!("python-tests: skip reason: {reason}")));
    if skip_count.is_some() && reasons.len() as u64 != skipped {
        return Err("skipped tests have missing reasons".into());
    }
    if skipped >= expected {
        return Err("all tests were skipped".into());
    }
    Ok(lines)
}

fn tests(ctx: &Context, root: &Path, directory: &str) -> Outcome {
    let selection = run::capture(ctx, root, "python3", &["-I", "-c", COUNT, directory]).map_err(|e| vec![e])?;
    if !selection.ok {
        return Err(vec![selection.stdout, selection.stderr]);
    }
    let expected: u64 = selection.stdout.trim().parse().map_err(|e| vec![format!("invalid test count: {e}")])?;
    if expected == 0 {
        return Err(vec![format!("{directory}: zero tests selected")]);
    }
    let out = run::capture(
        ctx,
        root,
        "python3",
        &["-I", "-m", "unittest", "discover", "-s", directory, "-p", "test_*.py", "-v"],
    )
    .map_err(|e| vec![e])?;
    let mut lines = vec![format!("{}{}", out.stdout, out.stderr)];
    if !out.ok {
        return Err(lines);
    }
    match summary(&out.stderr, expected) {
        Ok(counts) => {
            lines.extend(counts);
            Ok(lines)
        }
        Err(e) => {
            lines.push(e);
            Err(lines)
        }
    }
}

pub(crate) fn run(ctx: &Context, root: &Path) -> Outcome {
    validate_roots(ctx, root).map_err(|e| vec![e])?;
    let mut lines = Vec::new();
    for directory in ROOTS {
        match tests(ctx, root, directory) {
            Ok(result) => lines.extend(result),
            Err(result) => {
                lines.extend(result);
                return Err(lines);
            }
        }
    }
    Ok(lines)
}
