//! Clippy and rustc lints, counted per crate, file and lint for the ratchet.

use crate::ratchet::Ratchet;
use crate::run;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;

/// One line of `cargo clippy --message-format=json`, the fields the gate reads.
#[derive(Debug, Deserialize)]
struct Line {
    reason: String,
    manifest_path: Option<String>,
    message: Option<Diagnostic>,
}

#[derive(Debug, Deserialize)]
struct Diagnostic {
    level: String,
    code: Option<Code>,
    spans: Vec<Span>,
    rendered: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Code {
    code: String,
}

#[derive(Debug, Deserialize)]
struct Span {
    file_name: String,
    line_start: u64,
    column_start: u64,
    is_primary: bool,
}

/// The lint findings and compile errors of one clippy run.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Findings {
    /// `<crate> <file> <lint>`: count
    pub counts: Ratchet,
    pub errors: Vec<String>,
}

/// Reads clippy's JSON lines; only messages of the crate whose manifest is `manifest` count (path dependencies report
/// their own warnings too). A finding reported for the library and again for its test build counts once.
#[cfg(test)]
pub fn parse(json: &str, krate: &str, manifest: &str) -> Findings {
    let mut f = Findings::default();
    add(json, krate, manifest, &mut BTreeSet::new(), &mut f);
    f
}

/// A place a lint fired: file, line, column, lint.
type Place = (String, u64, u64, String);

/// Adds the findings of one clippy output to `f`; `seen` holds the places counted already, across runs.
fn add(json: &str, krate: &str, manifest: &str, seen: &mut BTreeSet<Place>, f: &mut Findings) {
    for l in json.lines().filter_map(|l| serde_json::from_str::<Line>(l).ok()) {
        let (Some(m), Some(mp)) = (l.message, l.manifest_path) else { continue };
        if l.reason != "compiler-message" || mp != manifest {
            continue;
        }
        if m.level == "error" {
            f.errors.push(m.rendered.unwrap_or_default());
            continue;
        }
        let (Some(code), Some(s)) = (m.code, m.spans.iter().find(|s| s.is_primary)) else { continue };
        if seen.insert((s.file_name.clone(), s.line_start, s.column_start, code.code.clone())) {
            *f.counts.0.entry(format!("{} {} {}", krate, s.file_name, code.code)).or_default() += 1;
        }
    }
}

/// Runs clippy on every target of the crate in `root/krate`, for the host and, with `scope`, for the scope's target
/// too: code under `cfg(target_os = "linux")` or `cfg(target_arch = "aarch64")` only builds there. A finding seen in
/// both counts once.
pub fn run(root: &Path, krate: &str, scope: bool) -> Result<Findings, String> {
    let dir = root.join(krate);
    let manifest = dir.join("Cargo.toml").to_string_lossy().into_owned();
    let mut f = Findings::default();
    let mut seen = BTreeSet::new();
    let host = ["clippy", "--locked", "--all-targets", "--message-format=json", "--quiet"];
    let mut runs = vec![host.to_vec()];
    if scope {
        runs.push([&host[..], &["--target", crate::repo::TARGET]].concat());
    }
    for args in runs {
        let out = run::capture(&dir, "cargo", &args)?;
        add(&out.stdout, krate, &manifest, &mut seen, &mut f);
        if !out.ok && f.errors.is_empty() {
            f.errors.push(run::tail(&out.stderr, 20));
        }
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(manifest: &str, level: &str, code: &str, file: &str, line: u64) -> String {
        format!(
            r#"{{"reason":"compiler-message","manifest_path":"{}","message":{{"level":"{}","code":{{"code":"{}"}},"spans":[{{"file_name":"{}","line_start":{},"column_start":1,"is_primary":true}}],"rendered":"r"}}}}"#,
            manifest, level, code, file, line
        )
    }

    #[test]
    fn counts_per_file_and_lint_once_per_place_and_only_for_the_crate() {
        let lines = [
            msg("/r/ui/Cargo.toml", "warning", "clippy::unwrap_used", "src/a.rs", 3),
            msg("/r/ui/Cargo.toml", "warning", "clippy::unwrap_used", "src/a.rs", 3),
            msg("/r/ui/Cargo.toml", "warning", "clippy::unwrap_used", "src/a.rs", 9),
            msg("/r/hal/Cargo.toml", "warning", "clippy::unwrap_used", "src/b.rs", 1),
            r#"{"reason":"build-finished","success":true}"#.to_string(),
        ];
        let f = parse(&lines.join("\n"), "ui", "/r/ui/Cargo.toml");
        assert_eq!(f.counts.0.get("ui src/a.rs clippy::unwrap_used"), Some(&2));
        assert_eq!(f.counts.0.len(), 1);
        assert!(f.errors.is_empty());
    }

    #[test]
    fn errors_are_kept_apart() {
        let f = parse(&msg("/m", "error", "E0308", "src/a.rs", 1), "x", "/m");
        assert_eq!(f.errors.len(), 1);
        assert!(f.counts.0.is_empty());
    }
}
