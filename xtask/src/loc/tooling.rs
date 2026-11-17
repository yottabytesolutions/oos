// SPDX-License-Identifier: GPL-2.0-or-later
//! Shell and Python lines, counted as tooling beside production code (ADR 0035). A line counts when it is neither
//! blank nor a `#` comment. Tests and test data do not count.

use std::collections::BTreeMap;
use std::path::Path;

/// The language of a file named `name` whose first line is `first`: by extension, or by the `#!` line of a file
/// without one.
pub fn kind(name: &str, first: &str) -> Option<&'static str> {
    if name.ends_with(".py") {
        Some("python")
    } else if name.ends_with(".sh") {
        Some("shell")
    } else if name.contains('.') || !first.starts_with("#!") {
        None
    } else if first.contains("python") {
        Some("python")
    } else if first.contains("sh") {
        Some("shell")
    } else {
        None
    }
}

/// Test code and test data: a test directory or a test file name.
pub fn is_test(path: &str) -> bool {
    let mut parts = path.split('/').rev();
    let name = parts.next().unwrap_or(path);
    let dirs = ["tests", "test", "testdata", "fixtures", "golden"];
    parts.any(|d| dirs.contains(&d)) || name.starts_with("test_") || name.ends_with("_test.py") || name == "conftest.py"
}

/// Lines that are neither blank nor a `#` comment.
pub fn lines(text: &str) -> u64 {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).count() as u64
}

/// Shell and Python lines among `files` under `root`. A file that is missing or not text is skipped.
pub fn count(root: &Path, files: &[String]) -> Result<BTreeMap<&'static str, u64>, String> {
    let mut out = BTreeMap::from([("python", 0), ("shell", 0)]);
    for f in files.iter().filter(|f| !is_test(f)) {
        let name = f.rsplit('/').next().unwrap_or(f);
        let path = root.join(f);
        let script = name.ends_with(".py") || name.ends_with(".sh") || !name.contains('.');
        if !script || !path.is_file() {
            continue;
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::InvalidData => continue,
            Err(e) => return Err(format!("{}: {}", f, e)),
        };
        if let Some(k) = kind(name, text.lines().next().unwrap_or("")) {
            *out.entry(k).or_default() += lines(&text);
        }
    }
    Ok(out)
}
