// SPDX-License-Identifier: GPL-2.0-or-later
//! `cargo xtask loc`: production code lines per crate and in total, and the tooling lines beside them (ADR 0035).
//! Production code is the Rust of the crates built for the scope and this project's C, outside test code, without
//! blank lines, comment lines and generated files. `--compare <ref>` measures `ref` as well and prints the change.

mod c;
mod report;
#[cfg(test)]
mod tests;
mod tooling;
mod tree;

use crate::repo::CRATES;
use std::collections::BTreeMap;
use std::path::Path;

const USAGE: &str = "usage: cargo xtask loc [--json] [--compare <git-ref>]";

/// The flags of `cargo xtask loc`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub json: bool,
    /// the git ref to compare with
    pub compare: Option<String>,
}

impl Options {
    pub fn parse(flags: &[&str]) -> Result<Options, String> {
        let mut o = Options::default();
        let mut it = flags.iter();
        while let Some(&f) = it.next() {
            match f {
                "--json" if !o.json => o.json = true,
                "--compare" if o.compare.is_none() => {
                    let r = it.next().filter(|r| !r.starts_with('-')).ok_or("--compare needs a git ref")?;
                    o.compare = Some(r.to_string());
                }
                _ => return Err(format!("unknown or repeated flag {}\n{}", f, USAGE)),
            }
        }
        Ok(o)
    }
}

/// Lines by name: crates and C directories for production code; `xtask`, `shell` and `python` for tooling.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Counts {
    pub production: BTreeMap<String, u64>,
    pub tooling: BTreeMap<String, u64>,
}

/// A generated file says so near its top with the common `@generated` marker.
fn generated(text: &str) -> bool {
    text.lines().take(5).any(|l| l.contains("@generated"))
}

/// A member built for the scope is production code, a host-only one (xtask) tooling. A member the gate does not
/// list counts as production code until it gets its entry in `CRATES`.
fn production(dir: &str) -> bool {
    CRATES.iter().find(|c| c.dir == dir).is_none_or(|c| c.target)
}

/// The directories holding a workspace: the root, or in a tree from before the root workspace (up to v0.1.13)
/// each top-level crate.
fn workspaces(files: &[String]) -> Vec<&str> {
    if files.iter().any(|f| f == "Cargo.toml") {
        return vec![""];
    }
    files.iter().filter_map(|f| f.strip_suffix("/Cargo.toml")).filter(|d| !d.contains('/')).collect()
}

/// Counts the tree at `root`, whose tracked files are `files`.
pub fn measure(root: &Path, files: &[String]) -> Result<Counts, String> {
    let mut counts = Counts::default();
    for w in workspaces(files) {
        let meta = crate::workspace::load(&Default::default(), &root.join(w))?;
        for p in &meta.packages {
            let sources = crate::sources::of_package(&meta, p, true)?;
            let n = sources.iter().filter(|s| !s.test && !generated(&s.text)).map(crate::size::code_lines).sum();
            let dir = [w, &meta.dir(p)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("/");
            let side = if production(&dir) { &mut counts.production } else { &mut counts.tooling };
            side.insert(dir, n);
        }
    }
    counts.production.extend(c::count(root, files)?);
    counts.tooling.extend(tooling::count(root, files)?.into_iter().map(|(k, n)| (k.to_string(), n)));
    Ok(counts)
}

/// The report for `flags`, as printed.
fn text(root: &Path, flags: &[&str]) -> Result<String, String> {
    let o = Options::parse(flags)?;
    let now = measure(root, &tree::files(root, None)?)?;
    let base = match &o.compare {
        Some(r) => Some((r.as_str(), tree::at(root, r, measure)?)),
        None => None,
    };
    let r = report::Report::new(&now, base.as_ref().map(|(r, c)| (*r, c)));
    if o.json {
        serde_json::to_string_pretty(&r).map(|s| s + "\n").map_err(|e| e.to_string())
    } else {
        Ok(r.markdown())
    }
}

/// `cargo xtask loc`: prints the report; false on an error.
pub fn run(root: &Path, flags: &[&str]) -> bool {
    match text(root, flags) {
        Ok(t) => {
            print!("{}", t);
            true
        }
        Err(e) => {
            eprintln!("{}", e);
            false
        }
    }
}
