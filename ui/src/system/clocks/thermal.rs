// SPDX-License-Identifier: GPL-2.0-or-later
//! Yield before a bound cooling trip or any zone's critical trip.
use super::status::Reason;
use super::Port;
use std::io;
use std::time::{Duration, Instant};

// Use the same 5 °C headroom for cooling and critical trips so a floor
// can yield before the trip despite sampling only once per second.
const TRIP_MARGIN: i64 = 5_000;
const REARM_HYSTERESIS: i64 = 5_000;
pub(super) const SAMPLE_PERIOD: Duration = Duration::from_secs(1);

struct Zone {
    kind: String,
    path: String,
    limit: i64,
}
impl Zone {
    fn discover(
        port: &dyn Port,
        name: &str,
        trip: impl Fn(&str, &str) -> io::Result<Option<i64>>,
    ) -> io::Result<Option<Self>> {
        let path = format!("class/thermal/{name}");
        let entries = port.entries(&path)?;
        let mut lowest = None;
        for entry in entries {
            if let Some(trip) = trip(&path, &entry)? {
                lowest = Some(lowest.map_or(trip, |other: i64| other.min(trip)));
            }
        }
        let Some(trip) = lowest else { return Ok(None) };
        let kind = port.read(&format!("{path}/type")).map_or_else(|_| name.to_string(), |kind| kind.trim().into());
        Ok(Some(Self { kind, path: format!("{path}/temp"), limit: trip.saturating_sub(TRIP_MARGIN) }))
    }

    fn blocks(&self, reading: &io::Result<i64>, rearm: bool) -> Option<Reason> {
        let threshold = self.limit.saturating_sub(if rearm { REARM_HYSTERESIS } else { 0 });
        match reading {
            Err(_) => Some(Reason::SensorError),
            Ok(temp) if *temp >= threshold => Some(Reason::Thermal),
            Ok(_) => None,
        }
    }
}

fn bound_trip(port: &dyn Port, zone: &str, entry: &str, cooling: &str) -> io::Result<Option<i64>> {
    let Some(slot) = entry.strip_prefix("cdev").and_then(|slot| slot.parse::<u32>().ok()) else {
        return Ok(None);
    };
    let link = format!("{zone}/cdev{slot}");
    let Ok(target) = port.read_link(&link) else { return Ok(None) };
    let cooling_link = target
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("cooling_device"))
        .is_some_and(|id| id.parse::<u32>().is_ok());
    if !cooling_link || !port.read(&format!("{link}/type")).is_ok_and(|kind| kind.trim() == cooling) {
        return Ok(None);
    }
    let trip = read_temp(port, &format!("{link}_trip_point"))?;
    // A negative trip index has no associated trip (THERMAL_TRIPS_NONE).
    if trip < 0 {
        return Ok(None);
    }
    let trip = format!("{zone}/trip_point_{trip}");
    match port.read(&format!("{trip}_type"))?.trim() {
        "passive" | "active" => read_temp(port, &format!("{trip}_temp")).map(Some),
        _ => Ok(None),
    }
}

fn critical_trip(port: &dyn Port, zone: &str, entry: &str) -> io::Result<Option<i64>> {
    let Some(trip) = entry
        .strip_suffix("_type")
        .filter(|trip| trip.strip_prefix("trip_point_").is_some_and(|index| index.parse::<u32>().is_ok()))
    else {
        return Ok(None);
    };
    if port.read(&format!("{zone}/{entry}"))?.trim() == "critical" {
        return read_temp(port, &format!("{zone}/{trip}_temp")).map(Some);
    }
    Ok(None)
}

pub(super) struct Guard {
    zones: Vec<Zone>,
    blocked: Option<Reason>,
    unavailable: Option<Reason>,
    next: Instant,
}
impl Guard {
    pub(super) fn binding(port: &dyn Port, device: &str, cooling: Option<&str>, now: Instant) -> Self {
        let zones = cooling.map_or_else(
            || Ok(Vec::new()),
            |cooling| discover_zones(port, |zone, entry| bound_trip(port, zone, entry, cooling)),
        );
        let (zones, unavailable) = match zones {
            Ok(zones) if zones.is_empty() => {
                if cooling.is_some() {
                    eprintln!("oos-ui: clock_floor device={device} no passive or active cooling binding; disabled");
                }
                (zones, Some(Reason::NoCoolingBinding))
            }
            Ok(zones) => (zones, None),
            Err(error) => {
                eprintln!("oos-ui: clock_thermal device={device} cooling_binding_error={error}; floor disabled");
                (Vec::new(), Some(Reason::CoolingBindingError))
            }
        };
        Self::new(device, zones, unavailable, now)
    }

    pub(super) fn critical(port: &dyn Port, now: Instant) -> Self {
        match discover_zones(port, |zone, entry| critical_trip(port, zone, entry)) {
            Ok(zones) => Self::new("all", zones, None, now),
            Err(error) => {
                eprintln!("oos-ui: clock_thermal device=all critical_trip_error={error}; floors disabled");
                Self::new("all", Vec::new(), Some(Reason::SensorError), now)
            }
        }
    }

    fn new(device: &str, zones: Vec<Zone>, unavailable: Option<Reason>, now: Instant) -> Self {
        for zone in &zones {
            eprintln!(
                "oos-ui: clock_thermal device={device} zone={} limit_mc={} hysteresis_mc={REARM_HYSTERESIS}",
                zone.kind, zone.limit
            );
        }
        Self { zones, blocked: None, unavailable, next: now }
    }

    pub(super) fn unavailable(&self) -> Option<Reason> {
        self.unavailable
    }

    pub(super) fn inhibit(&mut self, port: &dyn Port, device: &str, now: Instant) -> Option<Reason> {
        if self.zones.is_empty() {
            return self.unavailable;
        }
        if now < self.next {
            return self.blocked;
        }
        self.next = now + SAMPLE_PERIOD;
        let readings: Vec<_> = self.zones.iter().map(|zone| read_temp(port, &zone.path)).collect();
        let rearm = self.blocked.is_some();
        let samples = self.zones.iter().zip(&readings);
        let blocked = samples.clone().filter_map(|(zone, temp)| zone.blocks(temp, rearm)).max();
        if blocked.is_some() != rearm {
            for (zone, reading) in samples {
                if blocked.is_some() && zone.blocks(reading, rearm).is_none() {
                    continue;
                }
                let state = if blocked.is_some() { "dropped" } else { "rearmed" };
                match reading {
                    Ok(temp) => eprintln!(
                        "oos-ui: clock_thermal device={device} floors={state} zone={} temp_mc={temp}",
                        zone.kind
                    ),
                    Err(error) => eprintln!(
                        "oos-ui: clock_thermal device={device} floors={state} zone={} error={error}",
                        zone.kind
                    ),
                }
            }
        }
        self.blocked = blocked;
        blocked
    }
}

fn discover_zones(port: &dyn Port, trip: impl Fn(&str, &str) -> io::Result<Option<i64>>) -> io::Result<Vec<Zone>> {
    let mut zones = Vec::new();
    for name in port.entries("class/thermal")? {
        if name.strip_prefix("thermal_zone").is_some_and(|id| id.parse::<u32>().is_ok()) {
            zones.extend(Zone::discover(port, &name, &trip)?);
        }
    }
    Ok(zones)
}

fn read_temp(port: &dyn Port, path: &str) -> io::Result<i64> {
    port.read(path)?.trim().parse().map_err(io::Error::other)
}
