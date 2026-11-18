// SPDX-License-Identifier: GPL-2.0-or-later
//! Floors yield before Linux's first passive cooling trip; the kernel still owns cooling.
use super::Port;
use std::time::{Duration, Instant};
use std::{fmt, io};

// Policy margins in millidegrees C, relative to the running DT's sysfs passive trips.
const PASSIVE_MARGIN: i64 = 5_000;
const REARM_HYSTERESIS: i64 = 5_000;
pub(super) const SAMPLE_PERIOD: Duration = Duration::from_secs(1);

pub(super) enum Detail {
    MissingZones,
    NoPassiveTrip,
    Io(io::Error),
}
impl From<io::Error> for Detail {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl fmt::Display for Detail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingZones => f.write_str("missing-cpu-gpu-zones"),
            Self::NoPassiveTrip => f.write_str("no-passive-trip"),
            Self::Io(error) => write!(f, "io-{:?}", error.kind()),
        }
    }
}
pub(super) enum Reason {
    Limit,
    ReadError,
    Unavailable(Detail),
}
impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit => f.write_str("thermal-limit"),
            Self::ReadError => f.write_str("thermal-read-error"),
            Self::Unavailable(detail) => write!(f, "thermal-unavailable:{detail}"),
        }
    }
}

struct Zone {
    kind: String,
    path: String,
}
pub(super) struct Guard {
    zones: Vec<Zone>,
    limit: i64,
    blocked: bool,
    next: Instant,
    reason: Option<Reason>,
}
impl Guard {
    pub(super) fn discover(port: &dyn Port, now: Instant) -> Self {
        let (zones, limit, reason) = match Self::zones(port) {
            Ok((zones, limit)) => (zones, limit, None),
            Err(error) => {
                eprintln!("oos-ui: clock_thermal unavailable={error}; floors disabled");
                (Vec::new(), 0, Some(Reason::Unavailable(error)))
            }
        };
        Self { zones, limit, blocked: false, next: now, reason }
    }

    fn zones(port: &dyn Port) -> Result<(Vec<Zone>, i64), Detail> {
        let mut zones = Vec::new();
        let mut trips = Vec::new();
        for name in port.entries("class/thermal")? {
            if !name.starts_with("thermal_zone") {
                continue;
            }
            let path = format!("class/thermal/{name}");
            let kind = port.read(&format!("{path}/type"))?;
            if !matches!(kind.trim(), "cpu-thermal" | "gpu-thermal") {
                continue;
            }
            for file in port.entries(&path)? {
                let Some(prefix) = file.strip_suffix("_type").filter(|f| f.starts_with("trip_point_")) else {
                    continue;
                };
                if port.read(&format!("{path}/{file}"))?.trim() == "passive" {
                    trips.push(read_temp(port, &format!("{path}/{prefix}_temp"))?);
                }
            }
            zones.push(Zone { kind: kind.trim().to_string(), path: format!("{path}/temp") });
        }
        if !["cpu-thermal", "gpu-thermal"].iter().all(|kind| zones.iter().any(|zone| zone.kind == *kind)) {
            return Err(Detail::MissingZones);
        }
        // RK3399 can expose passive cooling only on CPU; both sensors share the earliest trip.
        let limit = trips.into_iter().min().ok_or(Detail::NoPassiveTrip)? - PASSIVE_MARGIN;
        for zone in &zones {
            eprintln!("oos-ui: clock_thermal zone={} limit_mc={limit} hysteresis_mc={REARM_HYSTERESIS}", zone.kind);
        }
        Ok((zones, limit))
    }

    fn blocks(&self, reading: &io::Result<i64>) -> bool {
        let threshold = self.limit - if self.blocked { REARM_HYSTERESIS } else { 0 };
        reading.as_ref().map_or(true, |temp| *temp >= threshold)
    }

    pub(super) fn reason(&self) -> Option<&Reason> {
        self.reason.as_ref()
    }

    pub(super) fn allowed(&mut self, port: &dyn Port, now: Instant) -> bool {
        if self.zones.is_empty() {
            return false;
        }
        if now < self.next {
            return !self.blocked;
        }
        self.next = now + SAMPLE_PERIOD;
        let readings: Vec<_> = self.zones.iter().map(|zone| read_temp(port, &zone.path)).collect();
        let blocked = readings.iter().any(|temp| self.blocks(temp));
        if blocked != self.blocked {
            for (zone, reading) in self.zones.iter().zip(&readings) {
                if blocked && !self.blocks(reading) {
                    continue;
                }
                let state = if blocked { "dropped" } else { "rearmed" };
                match reading {
                    Ok(temp) => eprintln!("oos-ui: clock_thermal floors={state} zone={} temp_mc={temp}", zone.kind),
                    Err(error) => eprintln!("oos-ui: clock_thermal floors={state} zone={} error={error}", zone.kind),
                }
            }
        }
        self.reason = if readings.iter().any(Result::is_err) {
            Some(Reason::ReadError)
        } else {
            blocked.then_some(Reason::Limit)
        };
        self.blocked = blocked;
        !blocked
    }
}

fn read_temp(port: &dyn Port, path: &str) -> io::Result<i64> {
    port.read(path)?.trim().parse().map_err(io::Error::other)
}
