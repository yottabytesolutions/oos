// SPDX-License-Identifier: GPL-2.0-or-later

mod content;

use super::{
    hex,
    process::{self, Scratch, Stderr},
    ssh::Remote,
};
use std::{
    fs::{self, File},
    io::Seek,
    path::Path,
    process::{Command, Stdio},
};

const CACHE: &str = "/tmp/oos-bench";
const INIT: &str = "
    umask 077
    [ ! -L /tmp/oos-bench ]
    mkdir -p /tmp/oos-bench
    [ \"$(stat -c %u:%a /tmp/oos-bench)\" = \"$(id -u):700\" ]
    cd /tmp/oos-bench
";
const STAGE: &str = "
    staging=$(mktemp -d .upload.XXXXXXXX)
    trap 'rm -rf \"$staging\"' EXIT
";

pub(super) fn groups(root: &Path, selected: &[&str]) -> Result<Vec<String>, String> {
    let mut groups: Vec<String> = selected.iter().map(|name| (*name).into()).collect();
    if groups.is_empty() {
        for entry in fs::read_dir(root).map_err(|_| "read fixture directory")? {
            let entry = entry.map_err(|_| "read fixture directory entry")?;
            if entry.path().is_dir() {
                groups.push(entry.file_name().into_string().map_err(|_| "fixture group names must be UTF-8")?);
            }
        }
    }
    groups.sort_unstable();
    if groups.is_empty() {
        return Err("fixture directory contains no groups".into());
    }
    if groups.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("fixture groups must be unique".into());
    }
    for name in &groups {
        if Path::new(name).file_name().and_then(|part| part.to_str()) != Some(name)
            || name.chars().any(char::is_control)
        {
            return Err("fixture group must be a directory name without control characters".into());
        }
        let metadata = fs::symlink_metadata(root.join(name)).map_err(|_| "read selected fixture group")?;
        if !metadata.is_dir() {
            return Err("each selected fixture group must be a real directory".into());
        }
    }
    Ok(groups)
}

pub(super) fn binary(remote: &mut impl Remote, scratch: &Scratch, path: &Path) -> Result<(String, String), String> {
    let mut source = File::open(path).map_err(|_| "open benchmark binary")?;
    let digest = hash(&mut source, scratch)?;
    let name = format!("bin-{digest}");
    let present = format!(
        "{INIT}[ ! -L {name} ]\nif [ -x {name} ]; then\n\
         checksum=$(sha256sum {name}); printf '%s\\n' \"${{checksum%% *}}\"; fi"
    );
    if remote.check_cache("check benchmark cache", &present)?.trim() != digest {
        let script = format!(
            "{INIT}{STAGE}
            [ ! -L {name} ]
            cat > \"$staging/bench\"
            [ \"$(sha256sum \"$staging/bench\" | cut -d ' ' -f 1)\" = {digest} ]
            chmod 700 \"$staging/bench\"
            mv -fT \"$staging/bench\" {name}"
        );
        remote.run("upload benchmark binary", &script, 120, Some(source))?;
    }
    Ok((format!("{CACHE}/{name}"), digest))
}

#[derive(Debug)]
pub(super) struct Fixture {
    pub(super) path: String,
    pub(super) sha256: String,
}

pub(super) fn fixture(remote: &mut impl Remote, scratch: &Scratch, path: &Path) -> Result<Fixture, String> {
    let sha256 = content::identity(path, scratch)?;
    let name = format!("group-{sha256}");
    let present = format!("{INIT}[ ! -L {name} ]\nif [ -d {name} ]; then echo present; fi");
    if remote.check_cache("check fixture cache", &present)? != "present\n" {
        let archive = archive(path, scratch)?;
        let script = format!(
            "{INIT}{STAGE}
            [ ! -L {name} ]
            mkdir \"$staging/fixtures\"
            tar -xf - -C \"$staging/fixtures\"
            mv -nT \"$staging/fixtures\" {name}
            [ -d {name} ] && [ ! -L {name} ]"
        );
        remote.run("upload fixture group", &script, 180, Some(archive))?;
    }
    Ok(Fixture { path: format!("{CACHE}/{name}"), sha256 })
}

fn archive(path: &Path, scratch: &Scratch) -> Result<File, String> {
    let mut archive = scratch.file("archive").map_err(|_| "open fixture archive")?;
    let sink = archive.try_clone().map_err(|_| "duplicate fixture archive")?;
    let mut tar = archive_command();
    tar.args(["-cf", "-", "-C"]).arg(path).arg(".").stdin(Stdio::null()).stdout(sink).stderr(Stdio::null());
    process::complete(&mut tar, "archive fixture group", 180)?;
    archive.rewind().map_err(|_| "rewind fixture archive for upload")?;
    Ok(archive)
}

fn archive_command() -> Command {
    // The host's tar formats differ. Keep local owner names and extra metadata out of the upload.
    let mut tar = if cfg!(target_os = "linux") {
        let mut tar = Command::new("tar");
        tar.args(["--format=gnu", "--numeric-owner", "--owner=0", "--group=0", "--sort=name"]);
        tar
    } else {
        let mut tar = Command::new("/usr/bin/tar");
        tar.args([
            "--format=gnutar",
            "--uid",
            "0",
            "--gid",
            "0",
            "--uname",
            "",
            "--gname",
            "",
            "--no-acls",
            "--no-xattrs",
            "--no-fflags",
            "--no-mac-metadata",
        ]);
        tar
    };
    tar.env_remove("TAR_OPTIONS");
    tar
}

fn hash(source: &mut File, scratch: &Scratch) -> Result<String, String> {
    source.rewind().map_err(|_| "rewind input for SHA-256")?;
    let digest = sha256(source.try_clone().map_err(|_| "duplicate input for SHA-256")?, scratch)?;
    source.rewind().map_err(|_| "rewind SHA-256 input for upload")?;
    Ok(digest)
}

fn sha256(input: impl Into<Stdio>, scratch: &Scratch) -> Result<String, String> {
    let mut command = if cfg!(target_os = "macos") {
        let mut shasum = Command::new("shasum");
        shasum.args(["-a", "256"]);
        shasum
    } else {
        Command::new("sha256sum")
    };
    let output = process::output(command.stdin(input), scratch, "compute SHA-256", 180, Stderr::Discard)?;
    output
        .split_ascii_whitespace()
        .next()
        .filter(|digest| hex(digest, 64))
        .map(str::to_owned)
        .ok_or_else(|| "invalid SHA-256 output".into())
}

#[cfg(test)]
mod tests;
