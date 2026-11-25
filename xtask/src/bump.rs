// SPDX-License-Identifier: GPL-2.0-or-later
//! Keep the release version and its generated references in sync.

mod files;
mod golden;
#[cfg(test)]
mod tests;

use crate::{run::Context, steps::Outcome};
use files::{Edit, Release};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask bump <X.Y.Z> [--allow-same] | cargo xtask bump --check";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Version([u64; 3]);

impl Version {
    fn parse(text: &str) -> Result<Self, Error> {
        let mut numbers = text.split('.');
        let mut version = [0; 3];
        for part in &mut version {
            let digits = numbers.next().ok_or(Error::Version)?;
            if !digits.bytes().all(|byte| byte.is_ascii_digit()) || (digits.starts_with('0') && digits.len() > 1) {
                return Err(Error::Version);
            }
            *part = digits.parse().map_err(|_| Error::Version)?;
        }
        if numbers.next().is_some() {
            return Err(Error::Version);
        }
        Ok(Self(version))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [major, minor, patch] = self.0;
        write!(f, "{major}.{minor}.{patch}")
    }
}

#[derive(Debug)]
enum Error {
    Version,
    Order(Version),
    File(PathBuf, std::io::Error),
    Format(String),
    Command(String),
    Restore(Box<Self>, Vec<Self>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Version => write!(f, "expected a release version in X.Y.Z form"),
            Self::Order(current) => {
                write!(f, "new version must be greater than {current}; use --allow-same to refresh {current}")
            }
            Self::File(path, error) => write!(f, "{}: {error}", path.display()),
            Self::Format(detail) | Self::Command(detail) => f.write_str(detail),
            Self::Restore(original, failures) => {
                write!(f, "{original}")?;
                for failure in failures {
                    write!(f, "\nrestore failed: {failure}")?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug)]
enum Request {
    Check,
    Bump { version: Version, allow_same: bool },
}

fn parse(flags: &[&str]) -> Result<Request, Error> {
    match flags {
        ["--check"] => Ok(Request::Check),
        [version] | [version, "--allow-same"] | ["--allow-same", version] if !version.starts_with('-') => {
            Ok(Request::Bump { version: Version::parse(version)?, allow_same: flags.len() == 2 })
        }
        _ => Err(Error::Format(USAGE.into())),
    }
}

pub(crate) fn run(root: &Path, flags: &[&str]) -> ExitCode {
    if flags == ["--help"] {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let result = match parse(flags) {
        Ok(Request::Check) => check(&Context::default(), root),
        Ok(Request::Bump { version, allow_same }) => {
            apply(root, version, allow_same, |root| golden::stage(root, version))
                .map(|count| vec![format!("release {version}: updated {count} files")])
                .map_err(|error| vec![error.to_string()])
        }
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for error in errors {
                eprintln!("{error}");
            }
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn check(ctx: &Context, root: &Path) -> Outcome {
    let release = Release::load(root).map_err(|error| vec![error.to_string()])?;
    let current = release.version;
    let mut errors = Vec::new();
    for result in release.checks(current) {
        match result {
            Ok(edit) if edit.changed() => errors.extend(edit.mismatches),
            Ok(_) => {}
            Err(error) => errors.push(error.to_string()),
        }
    }
    if let Err(found) = golden::check(ctx, root, current) {
        errors.extend(found);
    }
    crate::steps::outcome(vec![format!("release version {current} is consistent")], errors)
}

fn apply(
    root: &Path,
    version: Version,
    allow_same: bool,
    render: impl FnOnce(&Path) -> Result<Vec<Edit>, Error>,
) -> Result<usize, Error> {
    let _lock = golden::Scratch::create(root.join("target/release-bump.lock"))?;
    let release = Release::load(root)?;
    if version < release.version || (version == release.version && !allow_same) {
        return Err(Error::Order(release.version));
    }
    let (mut cargo, changelog) = release.edits(version)?;
    transact(root, &mut cargo, || {
        let mut edits = render(root)?;
        edits.push(changelog);
        Ok(edits)
    })
}

/// Cargo must see the new manifests and locks when it builds the renderer. Restore
/// them if building, rendering or publishing the staged references fails.
fn transact(
    root: &Path,
    cargo: &mut Vec<Edit>,
    stage: impl FnOnce() -> Result<Vec<Edit>, Error>,
) -> Result<usize, Error> {
    let mut written = 0;
    let result = (|| {
        cargo.retain(Edit::changed);
        for edit in cargo.iter() {
            written += 1;
            edit.write(root, &edit.after)?;
        }
        let mut staged = stage()?;
        staged.retain(Edit::changed);
        cargo.append(&mut staged);
        while let Some(edit) = cargo.get(written) {
            written += 1;
            edit.write(root, &edit.after)?;
        }
        Ok(cargo.len())
    })();
    result.map_err(|error| {
        let failures: Vec<_> =
            cargo[..written].iter().rev().filter_map(|edit| edit.write(root, &edit.before).err()).collect();
        if failures.is_empty() {
            error
        } else {
            Error::Restore(Box::new(error), failures)
        }
    })
}

fn read(root: &Path, path: &Path) -> Result<Vec<u8>, Error> {
    fs::read(root.join(path)).map_err(|error| Error::File(path.into(), error))
}
