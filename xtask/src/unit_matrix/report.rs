// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    plan::Setup,
    state::{Counters, SUMMARY_KEYS},
    window::{Rates, Window, WindowSample},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Report {
    pub(super) build: String,
    pub(super) base_setup: Setup,
    pub(super) bench_sha256: String,
    pub(super) window_s: f64,
    pub(super) skipped_setup_keys: Vec<String>,
    pub(super) groups: BTreeMap<String, GroupReport>,
    pub(super) error: Option<String>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct GroupReport {
    pub(super) overrides: Setup,
    pub(super) expected_windows: usize,
    pub(super) readback: Option<Readback>,
    pub(super) windows: Vec<Window>,
    pub(super) summary: Option<Summary>,
    pub(super) error: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Readback {
    pub(super) timebase: String,
    pub(super) memory_depth: String,
    pub(super) sample_rate: String,
    pub(super) acquisition_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Summary {
    pub(super) host_s: [f64; 2],
    pub(super) delta: Counters,
    pub(super) fallback_delta: Counters,
    pub(super) rates: Rates,
}

impl GroupReport {
    pub(super) fn summarize(&mut self) -> Result<(), String> {
        let mut valid = self.windows.iter().filter_map(|window| match &window.sample {
            WindowSample::Valid(sample) => Some((window.host_s, sample)),
            WindowSample::CounterReset => None,
        });
        let Some((mut host_s, first)) = valid.next() else { return Ok(()) };
        let mut delta = first.delta.clone();
        let mut fallback_delta = first.fallback_delta.clone();
        for (bounds, sample) in valid {
            for (total, elapsed) in host_s.iter_mut().zip(bounds) {
                *total += elapsed;
            }
            delta.retain(|key, total| {
                if let Some(change) = sample.delta.get(key) {
                    *total += change;
                    true
                } else {
                    false
                }
            });
            for (key, change) in &sample.fallback_delta {
                *fallback_delta.entry(key.clone()).or_default() += change;
            }
        }
        fallback_delta.retain(|_, value| *value != 0);
        let rates = Rates::calculate(host_s, &delta)?;
        delta.retain(|key, _| SUMMARY_KEYS.contains(&key.as_str()));
        self.summary = Some(Summary { host_s, delta, fallback_delta, rates });
        Ok(())
    }
}
