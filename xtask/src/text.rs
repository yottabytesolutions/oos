// SPDX-License-Identifier: GPL-2.0-or-later
//! The file rules of ADR 0019 a tool can check, over every tracked file: no trailing spaces or CR line ends, a
//! newline at the end, an SPDX line at the top of our own sources, Rust indented with spaces, and the licence texts
//! in the image tree equal to their originals.

use crate::steps::{outcome, Outcome};
use std::path::Path;

/// Taken from elsewhere and kept in their own format: the licence text, the XDMA driver, kernel patches, the recorded
/// kernel answers, firmware licences and the licence copies in the image tree.
const FOREIGN_FILES: [&str; 2] = ["LICENSE", "kernel/tools/git-log-answers.txt"];
const FOREIGN_DIRS: [&str; 3] = ["kernel/modules/", "kernel/patches/", "rootfs/initramfs/lib/firmware/"];
const LICENCE_DIR: &str = "rootfs/initramfs/etc/licenses/";

/// Each licence text the image carries (`/etc/licenses`) is a copy of a file in the tree: (original, copy).
const LICENCE_COPIES: [(&str, &str); 4] = [
    ("LICENSE", "rootfs/initramfs/etc/licenses/GPL-2.0.txt"),
    ("THIRD-PARTY-LICENSES.md", "rootfs/initramfs/etc/licenses/THIRD-PARTY-LICENSES.md"),
    ("ui/tools/fonts/LICENSE-Roboto.txt", "rootfs/initramfs/etc/licenses/Apache-2.0.txt"),
    ("kernel/firmware/LICENSE.wireless-regdb", "rootfs/initramfs/etc/licenses/wireless-regdb.txt"),
];

const SPDX: &[u8] = b"SPDX-License-Identifier:";
const SPDX_LINES: usize = 5;
const SOURCES: [&str; 5] = ["rs", "sh", "py", "c", "h"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    TrailingSpace,
    NoFinalNewline,
    NoSpdx,
    RustTabs,
}

impl Rule {
    fn says(self) -> &'static str {
        match self {
            Rule::TrailingSpace => "spaces or tabs at the end of a line (or CR line ends)",
            Rule::NoFinalNewline => "no newline at the end of the file",
            Rule::NoSpdx => "no SPDX-License-Identifier line in the first five lines",
            Rule::RustTabs => "Rust indented with tabs",
        }
    }
}

fn foreign(path: &str) -> bool {
    FOREIGN_FILES.contains(&path)
        || FOREIGN_DIRS.iter().any(|d| path.starts_with(d))
        || (path.starts_with(LICENCE_DIR) && path.ends_with(".txt"))
}

fn extension(path: &str) -> &str {
    Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("")
}

/// The rules the file `path` with the content `bytes` breaks. A file with a NUL byte is binary: only the SPDX rule
/// applies to it.
fn findings(path: &str, bytes: &[u8]) -> Vec<Rule> {
    let mut found = Vec::new();
    if foreign(path) {
        return found;
    }
    let text = !bytes.contains(&0);
    if text && bytes.split(|b| *b == b'\n').any(|l| l.last().is_some_and(|b| b" \t\r\x0b\x0c".contains(b))) {
        found.push(Rule::TrailingSpace);
    }
    if text && bytes.last().is_some_and(|b| *b != b'\n') {
        found.push(Rule::NoFinalNewline);
    }
    let ext = extension(path);
    if SOURCES.contains(&ext) && !bytes.split(|b| *b == b'\n').take(SPDX_LINES).any(|l| contains(l, SPDX)) {
        found.push(Rule::NoSpdx);
    }
    if ext == "rs" && bytes.split(|b| *b == b'\n').any(|l| l.first() == Some(&b'\t')) {
        found.push(Rule::RustTabs);
    }
    found
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// The content of the tracked file `path`, or `None` when it is gone from the work tree or is no regular file.
fn read(root: &Path, path: &str) -> Result<Option<Vec<u8>>, String> {
    let file = root.join(path);
    match std::fs::symlink_metadata(&file) {
        Ok(m) if m.is_file() => std::fs::read(&file).map(Some).map_err(|e| format!("{path}: {e}")),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{path}: {e}")),
    }
}

fn copies(root: &Path) -> Vec<String> {
    LICENCE_COPIES
        .iter()
        .filter(|(original, copy)| {
            !matches!((std::fs::read(root.join(original)), std::fs::read(root.join(copy))), (Ok(a), Ok(b)) if a == b)
        })
        .map(|(original, copy)| format!("{copy}: not a copy of {original} (THIRD-PARTY-LICENSES.md)"))
        .collect()
}

/// What the files `paths` break, one line per rule and file.
fn broken(root: &Path, paths: &[&String]) -> Vec<String> {
    let mut bad = Vec::new();
    for path in paths {
        match read(root, path) {
            Ok(Some(bytes)) => bad.extend(findings(path, &bytes).into_iter().map(|r| format!("{path}: {}", r.says()))),
            Ok(None) => {}
            Err(e) => bad.push(e),
        }
    }
    bad
}

pub fn check(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let paths = crate::repo::git_files(ctx, root, &[]).map_err(|e| vec![e])?;
    let paths: Vec<&String> = paths.iter().collect();
    let threads = std::thread::available_parallelism().map_or(1, usize::from);
    let mut bad: Vec<String> = std::thread::scope(|s| {
        let runs: Vec<_> =
            paths.chunks(paths.len().div_ceil(threads).max(1)).map(|c| s.spawn(move || broken(root, c))).collect();
        runs.into_iter().flat_map(|r| r.join().unwrap_or_else(|panic| std::panic::resume_unwind(panic))).collect()
    });
    bad.extend(copies(root));
    if !bad.is_empty() {
        bad.push("see docs/adr/process/0019-one-style-for-text-files-and-commits.md".into());
    }
    outcome(vec![format!("{} tracked files follow ADR 0019", paths.len())], bad)
}

#[cfg(test)]
mod tests;
