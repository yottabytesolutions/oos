// SPDX-License-Identifier: GPL-2.0-or-later

use super::state::{Counters, Sections, State};
use crate::unit_tools::scpi::{parse_state, Scpi, STATE};
use serde::{Deserialize, Serialize};
use std::{
    thread,
    time::{Duration, Instant},
};

pub(super) const COUNTER_RESET: &str = "counter reset";

pub(super) fn per_minute(count: f64, [low, high]: [f64; 2]) -> f64 {
    count / f64::midpoint(low, high) * 60.0
}

#[derive(Debug)]
pub(super) enum Failure {
    Unhealthy,
    Group(String),
}

impl Failure {
    pub(super) fn message(self) -> String {
        match self {
            Self::Unhealthy => "unit unhealthy: :SYSTem:STATe? lacks watchdog ok".into(),
            Self::Group(error) => error,
        }
    }
}

impl From<String> for Failure {
    fn from(error: String) -> Self {
        Self::Group(error)
    }
}

pub(super) trait Clock {
    fn elapsed(&self) -> Duration;
    fn wait(&mut self, duration: Duration);
}

pub(super) struct HostClock(pub(super) Instant);

impl Clock for HostClock {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }

    fn wait(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Window {
    pub(super) start_s: f64,
    pub(super) host_s: [f64; 2],
    pub(super) sample: WindowSample,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) enum WindowSample {
    #[serde(rename = "counter reset")]
    CounterReset,
    #[serde(rename = "valid")]
    Valid(Sample),
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Sample {
    pub(super) delta: Counters,
    pub(super) fallback_delta: Counters,
    pub(super) sections: Sections,
    pub(super) rates: Rates,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Rates {
    pub(super) fresh_fps: [f64; 2],
    pub(super) shown_records_per_s: f64,
    pub(super) first_records_per_s: f64,
    pub(super) read_per_s: f64,
    pub(super) late_flips: i128,
}

pub(super) fn measure(scpi: &mut impl Scpi, clock: &mut impl Clock, wait: Duration) -> Result<Window, Failure> {
    let a = scpi.query_timed(STATE, || clock.elapsed())?;
    let before = checked_state(&a.line)?;
    clock.wait(wait);
    let b = scpi.query_timed(STATE, || clock.elapsed())?;
    let after = checked_state(&b.line)?;
    let host_s = [(b.before_write - a.after_line).as_secs_f64(), (b.after_line - a.before_write).as_secs_f64()];
    let delta = diff(&before.counters, &after.counters);
    let mut fallback_delta = diff(&before.fallback, &after.fallback);
    fallback_delta.retain(|_, value| *value != 0);
    let sample = if delta.values().chain(fallback_delta.values()).any(|&value| value < 0) {
        WindowSample::CounterReset
    } else {
        let rates = Rates::calculate(host_s, &delta)?;
        WindowSample::Valid(Sample { delta, fallback_delta, sections: after.sections, rates })
    };
    Ok(Window { start_s: a.before_write.as_secs_f64(), host_s, sample })
}

fn checked_state(reply: &str) -> Result<State, Failure> {
    let state = State::parse(reply);
    if !state.healthy {
        return Err(Failure::Unhealthy);
    }
    if parse_state(reply) != Ok(true) {
        return Err("unit did not report RUN during the sampling window".to_owned().into());
    }
    Ok(state)
}

fn diff(before: &Counters, after: &Counters) -> Counters {
    after.iter().filter_map(|(key, &end)| Some((key.clone(), end - before.get(key)?))).collect()
}

impl Rates {
    pub(super) fn calculate([low, high]: [f64; 2], delta: &Counters) -> Result<Self, String> {
        if low <= 0.0 || high < low {
            return Err("invalid host-time bounds".into());
        }
        let count = |key: &str| {
            delta
                .get(key)
                .copied()
                .filter(|&count| count >= 0)
                .ok_or_else(|| format!("missing or reset counter '{key}'"))
        };
        let fresh = count("fresh")? as f64;
        let midpoint = f64::midpoint(low, high);
        Ok(Self {
            fresh_fps: [fresh / high, fresh / low],
            shown_records_per_s: count("records.shown")? as f64 / midpoint,
            first_records_per_s: count("first_records")? as f64 / midpoint,
            read_per_s: count("records.read")? as f64 / midpoint,
            late_flips: count("flip.late")?,
        })
    }
}
