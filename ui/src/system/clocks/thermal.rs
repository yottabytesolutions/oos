// SPDX-License-Identifier: GPL-2.0-or-later
//! Floors yield before Linux's first passive cooling trip; the kernel still owns cooling.
use super::Port;
use std::io;
use std::time::{Duration, Instant};

// Policy margins in millidegrees C, relative to the running DT's sysfs passive trips.
const PASSIVE_MARGIN: i64 = 5_000;
const REARM_HYSTERESIS: i64 = 5_000;
pub(super) const SAMPLE_PERIOD: Duration = Duration::from_secs(1);

struct Zone {
    kind: String,
    path: String,
    limit: i64,
}
impl Zone {
    fn blocks(&self, reading: &io::Result<i64>, blocked: bool) -> bool {
        let threshold = self.limit - if blocked { REARM_HYSTERESIS } else { 0 };
        reading.as_ref().map_or(true, |temp| *temp >= threshold)
    }
}
pub(super) struct Guard {
    zones: Vec<Zone>,
    blocked: bool,
    next: Instant,
}
impl Guard {
    pub(super) fn discover(port: &dyn Port) -> Self {
        let zones = Self::zones(port).unwrap_or_else(|error| {
            eprintln!("oos-ui: clock_thermal unavailable={error}; floors disabled");
            Vec::new()
        });
        Self { zones, blocked: false, next: Instant::now() }
    }

    fn zones(port: &dyn Port) -> io::Result<Vec<Zone>> {
        let mut zones = Vec::new();
        let mut kinds = Vec::new();
        for name in port.entries("class/thermal")? {
            let path = format!("class/thermal/{name}");
            if !name.starts_with("thermal_zone") {
                continue;
            }
            let kind = port.read(&format!("{path}/type"))?;
            if !matches!(kind.trim(), "cpu-thermal" | "gpu-thermal") {
                continue;
            }
            let mut trips = Vec::new();
            for file in port.entries(&path)? {
                let Some(prefix) = file.strip_suffix("_type").filter(|f| f.starts_with("trip_point_")) else {
                    continue;
                };
                if port.read(&format!("{path}/{file}"))?.trim() == "passive" {
                    trips.push(read_temp(port, &format!("{path}/{prefix}_temp"))?);
                }
            }
            let trip = trips.into_iter().min().ok_or_else(|| io::Error::other("SoC zone has no passive trip"))?;
            let limit = trip - PASSIVE_MARGIN;
            eprintln!("oos-ui: clock_thermal zone={} limit_mc={limit} hysteresis_mc={REARM_HYSTERESIS}", kind.trim());
            kinds.push(kind.trim().to_string());
            zones.push(Zone { kind: kind.trim().into(), path: format!("{path}/temp"), limit });
        }
        if !["cpu-thermal", "gpu-thermal"].iter().all(|kind| kinds.iter().any(|k| k == kind)) {
            return Err(io::Error::other("CPU/GPU thermal zones missing"));
        }
        Ok(zones)
    }

    pub(super) fn allowed(&mut self, port: &dyn Port) -> bool {
        if self.zones.is_empty() {
            return false;
        }
        let now = Instant::now();
        if now < self.next {
            return !self.blocked;
        }
        self.next = now + SAMPLE_PERIOD;
        let readings: Vec<_> = self.zones.iter().map(|zone| read_temp(port, &zone.path)).collect();
        let blocked = self.zones.iter().zip(&readings).any(|(zone, temp)| zone.blocks(temp, self.blocked));
        if blocked != self.blocked {
            for (zone, reading) in self.zones.iter().zip(&readings) {
                if blocked && !zone.blocks(reading, self.blocked) {
                    continue;
                }
                let state = if blocked { "dropped" } else { "rearmed" };
                match reading {
                    Ok(temp) => eprintln!("oos-ui: clock_thermal floors={state} zone={} temp_mc={temp}", zone.kind),
                    Err(error) => eprintln!("oos-ui: clock_thermal floors={state} zone={} error={error}", zone.kind),
                }
            }
        }
        self.blocked = blocked;
        !blocked
    }
}

fn read_temp(port: &dyn Port, path: &str) -> io::Result<i64> {
    port.read(path)?.trim().parse().map_err(io::Error::other)
}
