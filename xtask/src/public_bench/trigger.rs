// SPDX-License-Identifier: GPL-2.0-or-later

use super::{settings, state::read_state, Clock, Error, SETTLE};
use crate::unit_tools::scpi::Scpi;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub(super) enum Level {
    Auto,
    Explicit(f64),
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub(super) struct Trigger {
    #[serde(flatten)]
    method: Method,
    volts: f64,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "method", rename_all = "snake_case")]
enum Method {
    Explicit,
    MeasurementMidpoint { min_volts: f64, max_volts: f64 },
}

impl Trigger {
    pub(super) fn volts(&self) -> f64 {
        self.volts
    }

    pub(super) fn resolve(scope: &mut impl Scpi, clock: &impl Clock, signal: u8, level: Level) -> Result<Self, Error> {
        if let Level::Explicit(volts) = level {
            return Ok(Self { method: Method::Explicit, volts });
        }
        settings::prepare_trigger(scope, signal, None)?;
        if !advances_while_settling(scope, clock)? {
            return Err(Error::TriggerLevel);
        }
        let min_volts = measure(scope, "VMIN", signal)?;
        let max_volts = measure(scope, "VMAX", signal)?;
        if min_volts >= max_volts {
            return Err(Error::TriggerLevel);
        }
        Ok(Self { method: Method::MeasurementMidpoint { min_volts, max_volts }, volts: min_volts.midpoint(max_volts) })
    }
}

fn measure(scope: &mut impl Scpi, item: &str, signal: u8) -> Result<f64, Error> {
    let (reply, _) = settings::query_setting(scope, &format!(":MEAS:ITEM? {item},CHAN{signal}"))?;
    reply.as_deref().and_then(settings::finite).filter(|value| value.abs() < 9.9e37).ok_or(Error::TriggerLevel)
}

pub(super) fn advances_while_settling(scope: &mut impl Scpi, clock: &impl Clock) -> Result<bool, Error> {
    let (_, before) = read_state(scope, clock)?;
    clock.wait(SETTLE)?;
    let (_, after) = read_state(scope, clock)?;
    after.advanced_since(&before)
}
