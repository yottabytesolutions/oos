// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Plan, Setup};
use crate::unit_matrix::setup::{privacy::address_like, wire_value};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

impl Plan {
    pub(in crate::unit_matrix) fn replace_bench(&mut self, path: &Path) -> Result<(), String> {
        let data = fs::read(path).map_err(|_| "could not read --bench file")?;
        let setup: Setup =
            serde_json::from_slice(&data).map_err(|_| "--bench must contain a JSON object of strings")?;
        self.set_bench(setup)
    }

    pub(in crate::unit_matrix) fn bench_sha256(&self) -> Result<String, String> {
        // BTreeMap serialization makes the digest independent of key order and file formatting.
        let json = serde_json::to_vec(&self.bench_setup).map_err(|_| "could not encode bench setup")?;
        let mut command = if cfg!(target_os = "macos") {
            let mut command = Command::new("shasum");
            command.args(["-a", "256"]);
            command
        } else {
            Command::new("sha256sum")
        };
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|_| "could not start bench SHA-256 command")?;
        let written = child.stdin.take().is_some_and(|mut input| input.write_all(&json).is_ok());
        let output = child.wait_with_output().map_err(|_| "could not read bench SHA-256")?;
        if !written || !output.status.success() {
            return Err("could not compute bench SHA-256".into());
        }
        let digest = output
            .stdout
            .get(..64)
            .filter(|digest| digest.iter().all(u8::is_ascii_hexdigit))
            .ok_or("invalid bench SHA-256 output")?;
        String::from_utf8(digest.to_vec()).map_err(|_| "invalid bench SHA-256 output".into())
    }

    fn set_bench(&mut self, setup: Setup) -> Result<(), String> {
        if setup.keys().ne(self.bench_setup.keys()) {
            return Err("--bench must have the same keys as bench_setup in xtask/data/unit-matrix.json".into());
        }
        for (key, value) in &setup {
            if address_like(key, value) || wire_value(key, value).is_err() {
                return Err(format!("invalid --bench value for setup key '{key}'"));
            }
        }
        self.bench_setup = setup;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bench_replacement_changes_fingerprint_without_exposing_values() {
        let mut plan = Plan::load().unwrap();
        let original = plan.bench_sha256().unwrap();
        let mut setup = plan.bench_setup.clone();
        setup.insert("trig_level".into(), "5".into());
        plan.set_bench(setup.clone()).unwrap();
        assert_ne!(plan.bench_sha256().unwrap(), original);
        assert_eq!(plan.default_target()["trig_level"], "5");
        setup.insert("trig_level".into(), "example.invalid".into());
        assert_eq!(plan.set_bench(setup).unwrap_err(), "invalid --bench value for setup key 'trig_level'");
    }
}
