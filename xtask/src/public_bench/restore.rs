// SPDX-License-Identifier: GPL-2.0-or-later

use super::{settings, Error};
use crate::unit_tools::scpi::{parse_state, Scpi, STATE};

// Restore the model values, including the fields that the SCPI setters change
// indirectly. For example, setting the timebase scale also clears its fine adjustment.
const KEYS: &[&str] = &[
    "trig_src",
    "trig_la",
    "trig_level",
    "trig_level2",
    "trig_type",
    "trig_mode",
    "slope",
    "trig_coup",
    "trig_nrej",
    "holdoff",
    "acq",
    "avg_count",
    "mdepth",
    "auto_depth",
    "ch1.on",
    "ch2.on",
    "ch3.on",
    "ch4.on",
    "spec",
    "eye",
    "tdiv",
    "tfine",
    "delay",
    "zoom",
    "zoom_tdiv",
    "zdelay",
    "meas",
    "meas2",
    "cat",
    ":DVM:ENAB",
    ":COUN:ENAB",
    ":MEAS:STAT:DISP",
];

#[derive(Debug)]
pub(super) struct Snapshot {
    settings: Vec<(&'static str, String)>,
    running: bool,
}

impl Snapshot {
    pub(super) fn read(scope: &mut impl Scpi, initial_state: &str) -> Result<Self, Error> {
        let running = parse_state(initial_state).map_err(Error::Scpi)?;
        if read(scope, "trig_ref")? != "none" {
            return Err(Error::Measurement("select a live trigger source before benchmarking"));
        }
        let mut settings = Vec::with_capacity(KEYS.len());
        for &key in KEYS {
            let value = read(scope, key)?;
            match (key, value.as_str()) {
                ("trig_mode", "Single") => {
                    return Err(Error::Measurement(
                        "leave Single sweep before benchmarking to preserve its saved sweep",
                    ));
                }
                // Older builds may have no Auto-depth policy. Leave that setting alone.
                ("auto_depth", "") => continue,
                ("tfine" | "meas" | "meas2" | "cat", "") => {}
                (_, "") => return Err(Error::Snapshot(key)),
                _ => {}
            }
            settings.push((key, value));
        }
        Ok(Self { settings, running })
    }

    pub(super) fn has_auto_depth(&self) -> bool {
        self.settings.iter().any(|(key, _)| *key == "auto_depth")
    }

    pub(super) fn restore(&self, scope: &mut impl Scpi) -> Vec<&'static str> {
        let mut failures = 0;
        if !self.running {
            attempt(&mut failures, || restore_running(scope, false));
        }
        for (key, saved) in &self.settings {
            attempt(&mut failures, || {
                if read(scope, key)? != *saved {
                    let setter = if key.starts_with(':') {
                        format!("{key} {saved}")
                    } else {
                        let value = if *key == "tfine" && saved.is_empty() { "1" } else { saved };
                        format!(":SYST:SET {key} = {value}")
                    };
                    settings::write(scope, &setter)?;
                }
                Ok(())
            });
        }
        attempt(&mut failures, || restore_running(scope, self.running));
        let mut unverified = Vec::new();
        for (key, saved) in &self.settings {
            if attempt(&mut failures, || read(scope, key).map(|actual| actual == *saved)) != Some(true) {
                unverified.push(*key);
            }
        }
        if attempt(&mut failures, || running(scope).map(|actual| actual == self.running)) != Some(true) {
            unverified.push("RUN/STOP");
        }
        unverified
    }
}

fn read(scope: &mut impl Scpi, key: &'static str) -> Result<String, Error> {
    let query = if key.starts_with(':') { format!("{key}?") } else { format!(":SYST:GET? {key}") };
    let (value, _) = settings::query_setting(scope, &query)?;
    value
        .filter(|value| value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b" +-.:".contains(&byte)))
        .ok_or(Error::Snapshot(key))
}

fn running(scope: &mut impl Scpi) -> Result<bool, Error> {
    parse_state(&scope.query(STATE).map_err(Error::Scpi)?).map_err(|_| Error::InvalidReply(STATE))
}

fn restore_running(scope: &mut impl Scpi, saved: bool) -> Result<(), Error> {
    if running(scope)? != saved {
        settings::write(scope, if saved { ":RUN" } else { ":STOP" })?;
    }
    Ok(())
}

fn attempt<T>(failures: &mut usize, operation: impl FnOnce() -> Result<T, Error>) -> Option<T> {
    // TcpScpi reconnects after a transport error. Bound cleanup if the scope stays offline.
    if *failures == 3 {
        return None;
    }
    match operation() {
        Ok(result) => {
            *failures = 0;
            Some(result)
        }
        Err(Error::Scpi(_)) => {
            *failures += 1;
            None
        }
        Err(_) => {
            *failures = 0;
            None
        }
    }
}
