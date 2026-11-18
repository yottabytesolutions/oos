// SPDX-License-Identifier: GPL-2.0-or-later
//! Numeric decision citations and literal ADR paths, including prose in source comments.

use crate::links::{self, Tree};
use std::collections::BTreeMap;
use std::path::Path;

fn number(text: &str) -> Option<&str> {
    let digits = text.get(..4)?;
    (digits.bytes().all(|b| b.is_ascii_digit()) && !text.as_bytes().get(4).is_some_and(u8::is_ascii_digit))
        .then_some(digits)
}

fn records(paths: &[&String]) -> (BTreeMap<String, String>, Vec<String>) {
    let mut records = BTreeMap::new();
    let mut bad = Vec::new();
    for path in paths {
        let Some(relative) = path.strip_prefix("docs/adr/").filter(|n| n.ends_with(".md")) else {
            continue;
        };
        let name = relative.rsplit('/').next().expect("a path has a final component");
        let Some(id) = number(name).filter(|_| name.as_bytes().get(4) == Some(&b'-')) else {
            continue;
        };
        if relative.contains('/') {
            bad.push(format!("{path}: decision records belong directly in docs/adr/"));
        }
        if let Some(previous) = records.insert(id.to_string(), (*path).clone()) {
            bad.push(format!("duplicate ADR {id}: {previous} and {path}"));
        }
    }
    (records, bad)
}

fn citations(line: &str) -> impl Iterator<Item = (&str, &str)> {
    ["ADR ", "adr/", "architecture/"].into_iter().flat_map(move |prefix| {
        line.match_indices(prefix).filter_map(move |(at, _)| number(&line[at + prefix.len()..]).map(|id| (prefix, id)))
    })
}

fn paths(line: &str) -> impl Iterator<Item = &str> {
    line.split(|c: char| c.is_whitespace() || "`\"'()[]<>,".contains(c)).filter_map(|word| {
        let (_, tail) = word.split_once("adr/")?;
        let nested = tail
            .split_once('/')
            .is_some_and(|(category, _)| ["code", "hardware", "process", "system", "ui-ux"].contains(&category));
        (nested || (word.contains(".md") && number(tail.rsplit('/').next()?).is_some()))
            .then(|| word.trim_end_matches(['.', ';', ':']))
    })
}

fn check_steps(file: &str, text: &str) -> Vec<String> {
    let mut bad = Vec::new();
    let mut in_check = false;
    for (index, line) in text.lines().enumerate() {
        if line.starts_with("Check:") {
            in_check = true;
        } else if line.is_empty() {
            in_check = false;
        }
        if in_check {
            for id in line.split('`').skip(1).step_by(2) {
                if !crate::inventory::CHECKS.iter().any(|check| check.id == id) {
                    bad.push(format!("{file}:{}: unknown gate step {id}", index + 1));
                }
            }
        }
    }
    bad
}

fn references(tree: &Tree, records: &BTreeMap<String, String>, file: &str, text: &str) -> Vec<String> {
    let mut bad = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let location = format!("{file}:{}", index + 1);
        for (prefix, id) in citations(line) {
            if prefix == "architecture/" {
                bad.push(format!("{location}: legacy architecture/{id} citation; use ADR NNNN"));
            } else if !records.contains_key(id) {
                bad.push(format!("{location}: ADR {id} has no record"));
            }
        }
        for path in paths(line) {
            // Literal repository paths in code and prose start at the root; Markdown relative paths do not.
            let url = if path.starts_with("docs/adr/") { format!("/{path}") } else { path.to_string() };
            if let Some(why) = links::url_broken(tree, file, &url) {
                bad.push(format!("{location}: {why}"));
            }
        }
    }
    bad
}

pub(super) fn check(root: &Path, paths: &[&String]) -> Vec<String> {
    let (records, mut bad) = records(paths);
    let tree = Tree::new(paths.iter().map(|p| ((*p).clone(), None)));
    for path in paths {
        match std::fs::read(root.join(path)) {
            Ok(bytes) if !bytes.contains(&0) => {
                if let Ok(text) = std::str::from_utf8(&bytes) {
                    bad.extend(references(&tree, &records, path, text));
                    if path.starts_with("docs/adr/") && path.ends_with(".md") {
                        bad.extend(check_steps(path, text));
                    }
                }
            }
            Ok(_) => {}
            Err(e) => bad.push(format!("{path}: {e}")),
        }
    }
    bad
}

#[cfg(test)]
mod tests;
