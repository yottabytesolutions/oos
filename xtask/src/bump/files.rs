// SPDX-License-Identifier: GPL-2.0-or-later

mod lock;

use super::{Error, Version};
use crate::repo::CRATES;
use std::collections::BTreeSet;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(super) struct Edit {
    path: PathBuf,
    pub(super) before: Vec<u8>,
    pub(super) after: Vec<u8>,
    pub(super) mismatches: Vec<String>,
}

impl Edit {
    pub(super) fn new(path: impl Into<PathBuf>, before: Vec<u8>, after: Vec<u8>, mismatches: Vec<String>) -> Self {
        Self { path: path.into(), before, after, mismatches }
    }

    pub(super) fn changed(&self) -> bool {
        self.before != self.after || !self.mismatches.is_empty()
    }

    pub(super) fn write(&self, root: &Path, contents: &[u8]) -> Result<(), Error> {
        fs::write(root.join(&self.path), contents).map_err(|error| Error::File(self.path.clone(), error))
    }
}

#[derive(Debug)]
pub(super) struct Release<'a> {
    root: &'a Path,
    pub(super) version: Version,
}

impl<'a> Release<'a> {
    pub(super) fn load(root: &'a Path) -> Result<Self, Error> {
        let manifest = Manifest::load(root, PathBuf::from("xtask/Cargo.toml"))?;
        let version = Version::parse(&manifest.text[manifest.version])
            .map_err(|error| Error::Format(format!("xtask/Cargo.toml: {error}")))?;
        Ok(Self { root, version })
    }

    pub(super) fn checks(&self, version: Version) -> Vec<Result<Edit, Error>> {
        let (mut cargo, _) = self.cargo(version);
        cargo.push(changelog(self.root, version, true));
        cargo
    }

    pub(super) fn edits(&self, version: Version) -> Result<(Vec<Edit>, Edit), Error> {
        let (cargo, missing) = self.cargo(version);
        if !missing.is_empty() {
            return Err(Error::Format(missing.join("\n")));
        }
        Ok((cargo.into_iter().collect::<Result<_, _>>()?, changelog(self.root, version, false)?))
    }

    fn cargo(&self, version: Version) -> (Vec<Result<Edit, Error>>, Vec<String>) {
        let (mut changes, mut names) = (Vec::new(), BTreeSet::new());
        for krate in CRATES {
            let path = Path::new(krate.dir).join("Cargo.toml");
            changes.push(Manifest::load(self.root, path).map(|manifest| {
                names.insert(manifest.name.clone());
                manifest.edit(version)
            }));
        }
        let workspace = lock::load(self.root, "Cargo.lock");
        let deps = workspace.as_ref().ok().map(|text| lock::dependencies(text, &names)).unwrap_or_default();
        let fuzz = lock::load(self.root, "fuzz/Cargo.lock");
        let mut fuzz_names = fuzz.as_ref().ok().map(|text| lock::reachable(text, &deps, &names)).unwrap_or_default();
        fuzz_names.insert("oos-fuzz".into());
        let mut missing = Vec::new();
        for (path, source, required) in [("Cargo.lock", workspace, &names), ("fuzz/Cargo.lock", fuzz, &fuzz_names)] {
            changes.push(source.and_then(|text| {
                let (edit, absent) = lock::edit(path, text, &names, required, version)?;
                missing.extend(absent);
                Ok(edit)
            }));
        }
        (changes, missing)
    }
}

#[derive(Debug)]
struct Manifest {
    path: PathBuf,
    text: String,
    name: String,
    version: Range<usize>,
}

impl Manifest {
    fn load(root: &Path, path: PathBuf) -> Result<Self, Error> {
        let text = read_text(root, &path)?;
        let package = text.split_inclusive('\n').scan(0, |offset, line| {
            let start = *offset;
            *offset += line.len();
            Some((start, line))
        });
        let fields: Vec<_> = package
            .skip_while(|(_, line)| line.trim() != "[package]")
            .skip(1)
            .take_while(|(_, line)| !line.trim_start().starts_with('['))
            .collect();
        let field = |key| {
            fields
                .iter()
                .find_map(|(start, line)| quoted(line, key).map(|r| start + r.start..start + r.end))
                .ok_or_else(|| Error::Format(format!("{}: missing [package].{key}", path.display())))
        };
        let name = text[field("name")?].to_owned();
        let version = field("version")?;
        Ok(Self { path, text, name, version })
    }

    fn edit(self, version: Version) -> Edit {
        let mut after = self.text.clone();
        let old = &self.text[self.version.clone()];
        let details = mismatch(&self.path, &self.name, old, version);
        after.replace_range(self.version, &version.to_string());
        Edit::new(self.path, self.text.into_bytes(), after.into_bytes(), details.into_iter().collect())
    }
}

/// Only the quoted value is replaced. Comments, spacing and unrelated keys remain byte-identical.
fn quoted(line: &str, key: &str) -> Option<Range<usize>> {
    let (name, rest) = line.split_once('=')?;
    if name.trim() != key {
        return None;
    }
    let value = rest.trim_start();
    let quote = value.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    let start = line.len() - value.len() + 1;
    Some(start..start + value[1..].find(quote)?)
}

fn mismatch(path: &Path, item: &str, old: &str, version: Version) -> Option<String> {
    (Version::parse(old).ok() != Some(version))
        .then(|| format!("{}: {item} is {old}; expected {version}", path.display()))
}

fn read_text(root: &Path, path: &Path) -> Result<String, Error> {
    fs::read_to_string(root.join(path)).map_err(|error| Error::File(path.into(), error))
}

fn changelog(root: &Path, version: Version, check: bool) -> Result<Edit, Error> {
    let path = Path::new("CHANGELOG.md");
    let original = read_text(root, path)?;
    let heading = format!("## {version}");
    let mut after = original.clone();
    let released = original.lines().filter_map(release_heading).next();
    let mut mismatches = Vec::new();
    if check {
        if released != Some(version) {
            mismatches.push(format!(
                "CHANGELOG.md: latest release is {}; expected {version}",
                released.map_or_else(|| "missing".into(), |old| old.to_string())
            ));
        }
    } else if released != Some(version) {
        if let Some((offset, line)) = unreleased_heading(&original) {
            after.replace_range(offset..offset + line.len(), &heading);
        } else {
            let insert = original
                .find("\n## ")
                .or_else(|| original.find('\n'))
                .ok_or_else(|| Error::Format("CHANGELOG.md: missing title".into()))?;
            after.insert_str(insert + 1, &format!("{heading}\n\n"));
        }
    }
    Ok(Edit::new(path, original.into_bytes(), after.into_bytes(), mismatches))
}

fn unreleased_heading(text: &str) -> Option<(usize, &str)> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let heading = line.trim_end_matches(['\r', '\n']);
        if heading == "## Unreleased" {
            return Some((offset, heading));
        }
        offset += line.len();
    }
    None
}

fn release_heading(line: &str) -> Option<Version> {
    let title = line.strip_prefix("## ")?;
    let version = title.split([':', ' ']).next()?;
    Version::parse(version).ok()
}
