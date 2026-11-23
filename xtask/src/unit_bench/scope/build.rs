// SPDX-License-Identifier: GPL-2.0-or-later

use super::hex;
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug)]
pub(super) struct Build {
    pub(super) binary: PathBuf,
    pub(super) commit: String,
}

impl Build {
    pub(super) fn prepare(root: &Path, target: &Path) -> Result<Self, String> {
        let binary = build(root, target)?;
        Ok(Self { binary, commit: commit(root)? })
    }
}

fn build(root: &Path, target: &Path) -> Result<PathBuf, String> {
    let toolchain = toolchain(root)?;
    let output = Command::new("cargo")
        .arg(format!("+{toolchain}"))
        .args([
            "test",
            "--locked",
            "-p",
            "oos-ui",
            "--lib",
            "--release",
            "--target",
            crate::repo::TARGET,
            "--features",
            "private-picture-counts",
            "--no-run",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(root)
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_PROFILE_TEST_DEBUG", "0")
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("start benchmark build: {e}"))?;
    if !output.status.success() {
        return Err(format!("benchmark build failed with {}", output.status));
    }
    executable(&String::from_utf8_lossy(&output.stdout))
}

fn toolchain(root: &Path) -> Result<String, String> {
    let pins = std::fs::read_to_string(root.join("rust-toolchain.toml")).map_err(|_| "read rust-toolchain.toml")?;
    let channel = pins
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .skip_while(|line| *line != "[toolchain]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == "channel")
        .and_then(|(_, value)| value.trim().strip_prefix('"')?.strip_suffix('"'))
        .ok_or("rust-toolchain.toml must set a quoted toolchain channel")?;
    let parts: Vec<_> = channel.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit())) {
        return Err("rust-toolchain.toml must pin an exact Rust version".into());
    }
    Ok(channel.to_owned())
}

fn executable(cargo_json: &str) -> Result<PathBuf, String> {
    let mut artifacts = cargo_json
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|item| {
            item["reason"] == "compiler-artifact"
                && item["target"]["name"] == "oos_ui"
                && item["target"]["kind"].as_array().is_some_and(|kinds| kinds.iter().any(|kind| kind == "lib"))
                && item["profile"]["test"] == true
        })
        .filter_map(|item| item["executable"].as_str().map(PathBuf::from));
    match (artifacts.next(), artifacts.next()) {
        (Some(path), None) => Ok(path),
        _ => Err("Cargo did not identify exactly one oos-ui library test executable".into()),
    }
}

fn commit(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%H"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("read checkout commit: {e}"))?;
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || !hex(&commit, 40) {
        return Err("could not read checkout commit".into());
    }
    Ok(commit)
}

#[cfg(test)]
mod tests {
    use super::super::Scratch;
    use super::*;
    use std::fs;

    #[test]
    fn selects_only_cargos_new_library_test_artifact() {
        let lib = r#"{"reason":"compiler-artifact","target":{"name":"oos_ui","kind":["lib"]},"profile":{"test":true},"executable":"target/test-lib"}"#;
        let bin = lib.replace("\"kind\":[\"lib\"]", "\"kind\":[\"bin\"]").replace("test-lib", "test-bin");
        let non_test = lib.replace("true", "false").replace("test-lib", "regular-lib");
        assert_eq!(executable(&format!("{bin}\n{lib}\n{non_test}")).unwrap(), PathBuf::from("target/test-lib"));
        assert!(executable(&format!("{bin}\n{non_test}")).is_err());
        assert!(executable(&format!("{lib}\n{lib}")).is_err());
    }

    #[test]
    fn reads_the_toolchain_pin_from_the_checkout() {
        let scratch = Scratch::for_test();
        let pins = scratch.path().join("rust-toolchain.toml");
        fs::write(&pins, "[other]\nchannel = \"stable\"\n[toolchain]\nchannel = \"9.8.7\" # exact\n").unwrap();
        assert_eq!(toolchain(scratch.path()).unwrap(), "9.8.7");
        fs::write(pins, "[toolchain]\nchannel = \"stable\"\n").unwrap();
        assert!(toolchain(scratch.path()).unwrap_err().contains("exact Rust version"));
    }
}
