// SPDX-License-Identifier: GPL-2.0-or-later
//! Client entry files must load the shared repository rules.

use crate::steps::{outcome, Outcome};
use std::path::Path;

struct Pointer {
    path: &'static str,
    target: &'static str,
    prefix: &'static str,
}

const POINTERS: [Pointer; 4] = [
    Pointer { path: "CLAUDE.md", target: "AGENTS.md", prefix: "" },
    Pointer { path: "GEMINI.md", target: "AGENTS.md", prefix: "" },
    Pointer { path: ".github/copilot-instructions.md", target: "../AGENTS.md", prefix: "" },
    Pointer { path: ".cursor/rules/oos.mdc", target: "../../AGENTS.md", prefix: CURSOR },
];
const CURSOR: &str = "---\ndescription: OOS repository rules\nalwaysApply: true\n---\n\n";
const CLAUDE_TRAILER: &str = "\nEnd each Claude commit message with the `Claude-Session` trailer line the harness provides, followed by the session URL.";
const AIDER_PATH: &str = ".aider.conf.yml";
const AIDER: &str = "read:\n  - AGENTS.md";

/// Exact loader templates, a real root rules file and a closed inventory, with no allowances.
pub fn check(ctx: &crate::run::Context, root: &Path) -> Outcome {
    let mut bad = Vec::new();
    if let Err(e) = read(root, "AGENTS.md").and_then(|t| {
        if t.trim().is_empty() {
            Err("AGENTS.md: shared rules must be nonempty".into())
        } else {
            Ok(t)
        }
    }) {
        bad.push(e);
    }
    for pointer in POINTERS {
        if let Err(e) = markdown(root, &pointer) {
            bad.push(e);
        }
    }
    if let Err(e) = read(root, AIDER_PATH).and_then(|t| compare_template(AIDER_PATH, &normalize(&t), AIDER)) {
        bad.push(e);
    }
    match crate::repo::git_files(ctx, root, &["--cached", "--others", "--exclude-standard"]) {
        Ok(paths) => bad.extend(
            paths
                .into_iter()
                .filter(|p| recognized(p) && !supported(p))
                .map(|p| format!("{p}: unsupported agent entry file; extend the reviewed inventory first")),
        ),
        Err(e) => bad.push(e),
    }
    outcome(vec!["five client entry files load root AGENTS.md; no additional recognized entries".into()], bad)
}

fn supported(path: &str) -> bool {
    // Keep canonical spellings: a case alias could be a separate, unchecked file on a case-sensitive host.
    path == "AGENTS.md" || path == AIDER_PATH || POINTERS.iter().any(|p| path == p.path)
}

// Git already excludes ignored personal settings; discovery names also match on case-insensitive hosts.
fn recognized(path: &str) -> bool {
    let path = path.to_ascii_lowercase();
    let parts: Vec<_> = path.split('/').collect();
    let name = parts.last().copied().unwrap_or("");
    matches!(name, "agents.md" | "claude.md" | "claude.local.md" | "gemini.md" | ".aider.conf.yml" | ".cursorrules")
        || parts.iter().any(|p| matches!(*p, ".claude" | ".gemini"))
        || parts.windows(2).any(|p| p == [".cursor", "rules"])
        || parts.windows(2).any(|p| p[0] == ".github" && matches!(p[1], "prompts" | "agents" | "chatmodes"))
        || (parts.contains(&".github")
            && (name == "copilot-instructions.md"
                || (parts.windows(2).any(|p| p == [".github", "instructions"]) && name.ends_with(".instructions.md"))))
}

/// Reject links even in the entry file's parent directories before reading through them.
fn read(root: &Path, path: &str) -> Result<String, String> {
    let mut file = root.to_path_buf();
    for component in Path::new(path).components() {
        file.push(component);
        let meta = std::fs::symlink_metadata(&file).map_err(|e| format!("{path}: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err(format!("{path}: symlinks are not permitted in loader paths"));
        }
    }
    if !file.is_file() {
        return Err(format!("{path}: expected a regular file"));
    }
    std::fs::read_to_string(file).map_err(|e| format!("{path}: {e}"))
}

/// Only CRLF and the presence of one final newline are insignificant.
fn normalize(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    text.strip_suffix('\n').unwrap_or(&text).to_string()
}

fn compare_template(path: &str, text: &str, expected: &str) -> Result<(), String> {
    if text == expected {
        return Ok(());
    }
    let line = text
        .split('\n')
        .zip(expected.split('\n'))
        .position(|(actual, wanted)| actual != wanted)
        .unwrap_or_else(|| text.split('\n').count().min(expected.split('\n').count()))
        + 1;
    // Report the location without echoing potentially private content from an invalid loader.
    Err(format!("{path}: expected the exact loader template; first differing line {line}"))
}

fn markdown(root: &Path, pointer: &Pointer) -> Result<(), String> {
    let Pointer { path, target, prefix } = *pointer;
    let text = normalize(&read(root, path)?);
    let mut expected =
        format!("{prefix}Read [AGENTS.md]({target}) first. It is the single source of repository rules.");
    if path == "CLAUDE.md" && text != expected {
        expected.push('\n');
        expected.push_str(CLAUDE_TRAILER);
    }
    compare_template(path, &text, &expected)?;
    // The complete-text comparison also fixes the link spelling. Resolve that link from its actual directory.
    let parent = Path::new(path).parent().unwrap_or(Path::new(""));
    let resolved = root.join(parent).join(target).canonicalize().map_err(|e| format!("{path}: AGENTS.md link: {e}"))?;
    let shared = root.join("AGENTS.md").canonicalize().map_err(|e| format!("AGENTS.md: {e}"))?;
    if resolved != shared {
        return Err(format!("{path}: relative link must resolve to root AGENTS.md"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
