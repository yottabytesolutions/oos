// SPDX-License-Identifier: GPL-2.0-or-later

use super::{mismatch, quoted, read_text, Edit, Error, Version};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub(super) fn load(root: &Path, name: &str) -> Result<String, Error> {
    read_text(root, Path::new(name))
}

fn packages(text: &str) -> impl Iterator<Item = &str> {
    text.split_inclusive("\n[[package]]\n").skip(1)
}

fn field<'a>(block: &'a str, key: &str) -> Option<&'a str> {
    block.lines().find_map(|line| quoted(line, key).map(|range| &line[range]))
}

fn local(block: &str) -> Option<&str> {
    if field(block, "source").is_some() {
        return None;
    }
    field(block, "name")
}

fn references(block: &str) -> impl Iterator<Item = &str> {
    block.lines().filter_map(reference)
}

fn reference(line: &str) -> Option<&str> {
    line.trim().strip_prefix('"')?.strip_suffix("\",")
}

pub(super) fn dependencies<'a>(text: &'a str, names: &BTreeSet<String>) -> BTreeMap<&'a str, Vec<&'a str>> {
    packages(text)
        .filter_map(|block| Some((local(block)?, references(block).collect())))
        .filter(|(name, _)| names.contains(*name))
        .collect()
}

pub(super) fn reachable(fuzz: &str, deps: &BTreeMap<&str, Vec<&str>>, names: &BTreeSet<String>) -> BTreeSet<String> {
    let mut pending: Vec<_> =
        packages(fuzz).find(|block| local(block) == Some("oos-fuzz")).into_iter().flat_map(references).collect();
    let mut found = BTreeSet::new();
    while let Some(reference) = pending.pop() {
        if let Some(name) = release_reference(reference, names) {
            if found.insert(name.to_owned()) {
                pending.extend(deps.get(name).into_iter().flatten().copied());
            }
        }
    }
    found
}

fn release_reference<'a>(reference: &'a str, names: &BTreeSet<String>) -> Option<&'a str> {
    let mut parts = reference.split_whitespace();
    let name = parts.next()?;
    // A source-qualified reference refers to a registry or Git package.
    (names.contains(name) && parts.nth(1).is_none()).then_some(name)
}

pub(super) fn edit(
    name: &str,
    text: String,
    names: &BTreeSet<String>,
    required: &BTreeSet<String>,
    version: Version,
) -> Result<(Edit, Vec<String>), Error> {
    let path = Path::new(name);
    let boundary = "\n[[package]]\n";
    let external: BTreeSet<_> = packages(&text)
        .filter(|block| field(block, "source").is_some())
        .filter_map(|block| Some((field(block, "name")?, field(block, "version")?)))
        .collect();
    let mut blocks = text.split_inclusive(boundary);
    let mut after = blocks.next().ok_or_else(|| Error::Format(format!("{name}: empty lockfile")))?.to_owned();
    let (mut found, mut mismatches) = (BTreeSet::new(), Vec::new());
    for block in blocks {
        let local = local(block);
        if let Some(package) = local {
            if !found.insert(package.to_owned()) {
                return Err(Error::Format(format!("{name}: duplicate local package {package}")));
            }
        }
        let package = local.filter(|name| names.contains(*name));
        if let Some(package) = package {
            if field(block, "version").is_none() {
                return Err(Error::Format(format!("{name}: missing version for {package}")));
            }
        }
        for line in block.split_inclusive('\n') {
            let qualified = reference(line)
                .filter(|key| release_reference(key, names).is_some())
                .and_then(|key| key.split_once(' '))
                .filter(|key| !external.contains(key));
            after.push_str(&edit_line(path, package, line, qualified, version, &mut mismatches));
        }
    }
    let missing: Vec<_> =
        required.difference(&found).map(|package| format!("{name}: missing local package {package}")).collect();
    mismatches.extend(missing.iter().cloned());
    Ok((Edit::new(path, text.into_bytes(), after.into_bytes(), mismatches), missing))
}

fn edit_line(
    path: &Path,
    package: Option<&str>,
    line: &str,
    qualified: Option<(&str, &str)>,
    version: Version,
    mismatches: &mut Vec<String>,
) -> String {
    if let Some((package, range)) = package.zip(quoted(line, "version")) {
        mismatches.extend(mismatch(path, package, &line[range.clone()], version));
        let mut after = line.to_owned();
        after.replace_range(range, &version.to_string());
        return after;
    }
    if let Some((name, old)) = qualified {
        mismatches.extend(mismatch(path, &format!("{name} dependency"), old, version));
        return line.replacen(&format!("\"{name} {old}\""), &format!("\"{name} {version}\""), 1);
    }
    line.to_owned()
}
