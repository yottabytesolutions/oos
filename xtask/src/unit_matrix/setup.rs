// SPDX-License-Identifier: GPL-2.0-or-later

pub(super) mod privacy;

use super::{plan::Setup, state::State};
use crate::unit_tools::scpi::{parse_state, Scpi, STATE};

const ERROR: &str = ":SYSTem:ERRor?";
const NO_ERROR: &str = "0,\"No error\"";
const RESTORE_TRANSPORT_LIMIT: usize = 3;

#[derive(Debug)]
pub(super) struct Snapshot {
    setup: Setup,
    running: bool,
    pub(super) skipped_keys: Vec<String>,
}

impl Snapshot {
    pub(super) fn read<'a>(scpi: &mut impl Scpi, keys: impl Iterator<Item = &'a String>) -> Result<Self, String> {
        let mut client = SetupScpi::new(scpi);
        let mut setup = Setup::new();
        let mut skipped_keys = Vec::new();
        for key in keys {
            let value = client.get(key)?;
            if privacy::address_like(key, &value) {
                eprintln!("skip setup key '{key}': address-like value");
                skipped_keys.push(key.clone());
            } else {
                setup.insert(key.clone(), value);
            }
        }
        skipped_keys.sort();
        let state = client.query(STATE).map_err(|_| "could not read state before setup")?;
        if !State::parse(&state).healthy {
            return Err("unit unhealthy before setup: :SYSTem:STATe? lacks watchdog ok".into());
        }
        Ok(Self { setup, running: parse_state(&state)?, skipped_keys })
    }

    pub(super) fn check_replayable(&self, base: &Setup) -> Result<(), String> {
        if self.running && self.setup.get("trig_mode").is_some_and(|mode| mode == "Single") {
            return Err("finish the armed SINGLE acquisition before starting the matrix".into());
        }
        for (key, value) in &self.setup {
            if let Some(expected) = base.get(key) {
                wire_value(key, value)?;
                if (key == "dec_sel" && value != expected)
                    || (decoder_slot(key) && expected.is_empty() && !value.is_empty())
                {
                    return Err(format!("setup key '{key}' is incompatible with the matrix decoder layout"));
                }
            }
        }
        Ok(())
    }

    pub(super) fn recovery_hint(&self) -> String {
        let mode = if self.running { "RUN" } else { "STOP" };
        format!("If interrupted or restoration fails, recall the setup saved before this run and select {mode}.")
    }

    pub(super) fn apply(&self, scpi: &mut impl Scpi, target: &Setup) -> Result<(), String> {
        let mut client = SetupScpi::new(scpi);
        for (key, value) in ordered(target).into_iter().filter(|(key, _)| self.setup.contains_key(*key)) {
            if client.set_if_changed(key, value)? && client.get(key)? != *value {
                return Err(format!("set '{key}': read-back mismatch"));
            }
        }
        Ok(())
    }

    pub(super) fn restore(&self, scpi: &mut impl Scpi) -> Result<(), String> {
        let mut client = SetupScpi::new(scpi);
        let ordered = ordered(&self.setup);
        'passes: for _ in 0..2 {
            // A setup change may clamp another key. The second pass reapplies anything that moved.
            for (key, value) in &ordered {
                let _ = client.set_if_changed(key, value);
                if client.exhausted() {
                    break 'passes;
                }
            }
        }
        let run = client.set_run(self.running);
        let mut restored = Vec::new();
        let mut unverified = Vec::new();
        for (key, value) in &self.setup {
            let verified = client.get(key).is_ok_and(|current| current == *value);
            record_verification(key, verified, &mut restored, &mut unverified);
        }
        let run_matches = client.query(STATE).ok().and_then(|reply| parse_state(&reply).ok()) == Some(self.running);
        record_verification("RUN/STOP", run_matches, &mut restored, &mut unverified);
        if unverified.is_empty() && run.is_ok() {
            return Ok(());
        }
        let stopped = if client.exhausted() { "; stopped after three consecutive SCPI transport failures" } else { "" };
        let run_error = if run.is_err() { "; RUN/STOP command failed or was not sent" } else { "" };
        Err(format!(
            "restore failed{stopped}{run_error}; verified restored keys: {}; mismatched or unverified keys: {}. {}",
            list_keys(&restored),
            list_keys(&unverified),
            self.recovery_hint()
        ))
    }
}

fn record_verification<'a>(key: &'a str, verified: bool, restored: &mut Vec<&'a str>, unverified: &mut Vec<&'a str>) {
    if verified {
        restored.push(key);
    } else {
        unverified.push(key);
    }
}

fn list_keys(keys: &[&str]) -> String {
    if keys.is_empty() {
        "none".into()
    } else {
        keys.join(", ")
    }
}

fn ordered(setup: &Setup) -> Vec<(&String, &String)> {
    let mut keys: Vec<_> = setup.iter().collect();
    keys.sort_by_key(|(key, _)| match key.as_str() {
        "dec_sel" | "math_sel" | "trig_type" | "views" => 0,
        "zoom_tdiv" | "meas" => 2,
        "zdelay" | "hdmi_view" => 3,
        key if key.ends_with(".offset") || key == "math_offset" => 4,
        _ => 1,
    });
    keys
}

struct SetupScpi<'a, S> {
    scpi: &'a mut S,
    failures: usize,
}

impl<'a, S: Scpi> SetupScpi<'a, S> {
    fn new(scpi: &'a mut S) -> Self {
        Self { scpi, failures: 0 }
    }

    fn exhausted(&self) -> bool {
        self.failures >= RESTORE_TRANSPORT_LIMIT
    }

    fn query(&mut self, command: &str) -> Result<String, ()> {
        if self.exhausted() {
            return Err(());
        }
        match self.scpi.query(command) {
            Ok(line) => {
                self.failures = 0;
                Ok(line)
            }
            Err(_) => {
                self.failures += 1;
                Err(())
            }
        }
    }

    fn get(&mut self, key: &str) -> Result<String, String> {
        self.query(&format!(":SYSTem:GET? {key}"))
            .map_err(|()| format!("read setup key '{key}': SCPI transport failure"))
    }

    fn set_if_changed(&mut self, key: &str, desired: &str) -> Result<bool, String> {
        if self.get(key)? == desired {
            return Ok(false);
        }
        let value = wire_value(key, desired)?;
        self.complete(&format!(":SYSTem:SET {key} = {value};*OPC?")).map_err(|e| format!("set '{key}': {e}"))?;
        Ok(true)
    }

    fn set_run(&mut self, running: bool) -> Result<(), String> {
        let mode = if running { "RUN" } else { "STOP" };
        self.complete(&format!(":{mode};*OPC?")).map_err(|e| format!("set {mode}: {e}"))
    }

    fn complete(&mut self, command: &str) -> Result<(), String> {
        let complete = self.query(command).map_err(|()| "SCPI transport failure")?;
        let queue = self.query(ERROR).map_err(|()| "SCPI error queue transport failure")?;
        if complete != "1" {
            return Err("SCPI operation did not complete".into());
        }
        if queue != NO_ERROR {
            let code = queue.split_once(',').and_then(|(code, _)| code.parse::<i32>().ok());
            return Err(code
                .map_or_else(|| "could not verify the SCPI error queue".into(), |code| format!("SCPI error {code}")));
        }
        Ok(())
    }
}

pub(super) fn wire_value<'a>(key: &str, value: &'a str) -> Result<&'a str, String> {
    if value.contains(';') || value.chars().any(char::is_control) {
        return Err(format!("setup key '{key}' cannot be replayed through SCPI"));
    }
    if !value.is_empty() {
        return Ok(value);
    }
    // GET hides these defaults. Send the setup codec's spelling when returning to one.
    let value = match key {
        "cur_b_source" => "none",
        key if key.ends_with(".unit") => "V",
        key if key.ends_with(".vfine") || key == "tfine" => "1",
        key if key.ends_with(".bias") || key.ends_with(".skew") || key.starts_with("deskew_") => "0",
        _ => value,
    };
    Ok(value)
}

fn decoder_slot(key: &str) -> bool {
    key.as_bytes().get(..3).is_some_and(|prefix| matches!(prefix, [b'd', b'1'..=b'4', b'.']))
}

pub(super) fn set_run(scpi: &mut impl Scpi, running: bool) -> Result<(), String> {
    SetupScpi::new(scpi).set_run(running)
}
