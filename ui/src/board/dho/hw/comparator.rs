// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Mask and zone requests and the stall guard for their shared hardware unit.

#[cfg(any(target_os = "linux", test))]
use oos_hal::dho::{auxout, seq::Settings};
use oos_hal::sync::lock;

/// The `mask.hw` line of system.conf; off unless `on`.
pub(in crate::board::dho) fn mask_hw_from(conf: &str) -> bool {
    crate::drawpipe::conf_value(None, conf, "mask.hw").as_deref() == Some("on")
}

/// The `zone.hw` line of system.conf; off unless `on`.
pub(in crate::board::dho) fn zone_hw_from(conf: &str) -> bool {
    crate::drawpipe::conf_value(None, conf, "zone.hw").as_deref() == Some("on")
}

/// Keys of the comparator mapping in system.conf: `mask.map.<key> = <value>` (`mask::Map::set`).
const MASK_MAP_KEYS: [&str; 8] = ["domain", "cpd", "mid", "shift", "offs", "invert", "cols", "range"];

/// The mapping of system.conf's `mask.map.*` lines over the default; bad lines are reported and skipped.
pub(in crate::board::dho) fn mask_map_from(conf: &str) -> oos_hal::dho::mask::Map {
    let mut m = oos_hal::dho::mask::Map::default();
    for k in MASK_MAP_KEYS {
        if let Some(v) = crate::drawpipe::conf_value(None, conf, &format!("mask.map.{}", k)) {
            if let Err(e) = m.set(&format!("{}={}", k, v)) {
                eprintln!("oos-ui: system.conf mask.map.{}: {}", k, e);
            }
        }
    }
    m
}

/// The mapping under test: `Map::default()` until the acquisition thread starts with the switches' mapping
/// (`mask_map_init`); no file is read here.
fn mask_map_cell() -> &'static std::sync::Mutex<oos_hal::dho::mask::Map> {
    static MAP: std::sync::OnceLock<std::sync::Mutex<oos_hal::dho::mask::Map>> = std::sync::OnceLock::new();
    MAP.get_or_init(Default::default)
}

/// Starts the mapping under test at `m` (`BoardSwitches::mask_map`, read at start).
#[cfg(target_os = "linux")]
pub(in crate::board::dho) fn mask_map_init(m: oos_hal::dho::mask::Map) {
    *oos_hal::sync::lock(mask_map_cell()) = m;
}

/// Tests that set another mapping under test hold this while they do, and put the default back.
#[cfg(test)]
pub fn mask_map_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    oos_hal::sync::lock(&LOCK)
}

/// The comparator mapping of the mask test and the zone trigger now (`mask_map_init`, then `mask_map_set`).
pub fn mask_map() -> oos_hal::dho::mask::Map {
    *lock(mask_map_cell())
}

/// Sets one `key=value` of the mapping (None: none), and returns the whole mapping. The acquisition thread takes the
/// new mapping with the next batch and reloads the tables.
pub fn mask_map_set(kv: Option<&str>) -> Result<String, String> {
    let mut m = lock(mask_map_cell());
    if let Some(kv) = kv {
        m.set(kv)?;
    }
    Ok(m.text())
}

/// What a stall of the mask/zone unit is kept for: time base, depth, channel set and comparator mapping. Another of
/// any of them tries the hardware again.
#[cfg(any(target_os = "linux", test))]
type UnitKey = (u64, i128, u32, oos_hal::dho::mask::Map);

#[cfg(any(target_os = "linux", test))]
pub(super) fn unit_key(s: &Settings, map: oos_hal::dho::mask::Map) -> UnitKey {
    (s.tdiv.to_bits(), s.depth, s.mask, map)
}

/// Consecutive failed arms ("no acquisition") with the mask/zone unit running after which it is dropped for these
/// settings (round 8: 200 us/div at 100k and Auto stopped the acquisition for good).
#[cfg(any(target_os = "linux", test))]
const UNIT_STALLS: u32 = 2;

/// A first block lapses after this and the hardware is tried once more: stalls during a settings change can land on
/// the new settings (round 19: switching from a stalled 10k map to a 100k record blocked the 100k record). A second
/// block of the same settings stays.
#[cfg(any(target_os = "linux", test))]
pub(super) const UNIT_RETRY: std::time::Duration = std::time::Duration::from_secs(5);

/// An arm that found no acquisition within the poll time (`seq`'s 0x4080 poll).
#[cfg(any(target_os = "linux", test))]
pub(super) fn is_stall(e: &str) -> bool {
    e.contains("no acquisition")
}

/// Whether a failed batch goes to the log. No acquisition without the mask/zone unit is the ordinary wait for a
/// trigger in Normal (round 21: one line every 1.5 s); with the unit running it is a stall and logged.
#[cfg(any(target_os = "linux", test))]
pub(super) fn logs_acquire_error(stalled: bool, unit: bool) -> bool {
    !stalled || unit
}

/// The stall guard of the mask/zone unit: counts arms in a row that found no acquisition with the unit running and
/// the same settings, and blocks the unit for those settings. After every stall one batch runs without the unit: the
/// read-back state machine stays in state 5 across a reconfiguration (round 20), so new settings armed right after a
/// stall stalled as well.
#[derive(Default)]
#[cfg(any(target_os = "linux", test))]
pub(super) struct UnitGuard {
    /// the settings of the last stall and the stalls in a row with them
    pub(super) stalls: Option<(UnitKey, u32)>,
    /// blocked settings, when, and whether the block already lapsed once for them
    blocked: Option<(UnitKey, std::time::Instant, bool)>,
    /// the next batch runs without the unit
    pub(super) flush: bool,
}

#[cfg(any(target_os = "linux", test))]
impl UnitGuard {
    /// A batch ended: `stalled` = no acquisition, `unit` = the mask or zone unit was running. Returns true when the
    /// unit is now blocked for `key`.
    pub(super) fn batch(&mut self, stalled: bool, unit: bool, key: UnitKey, now: std::time::Instant) -> bool {
        if !unit {
            // the flush batch (or no unit at all): the stall count stays
            self.flush = false;
            return false;
        }
        if !stalled {
            self.stalls = None;
            return false;
        }
        self.flush = true;
        let n = match self.stalls {
            Some((k, n)) if k == key => n + 1,
            _ => 1,
        };
        if n < UNIT_STALLS {
            self.stalls = Some((key, n));
            return false;
        }
        self.stalls = None;
        let again = self.blocked.as_ref().is_some_and(|b| b.0 == key);
        self.blocked = Some((key, now, again));
        true
    }

    /// Whether the unit stays off for `key` at `now`: the flush batch after a stall, or a block (a first block lapses
    /// after `UNIT_RETRY`).
    pub(super) fn blocks(&self, key: &UnitKey, now: std::time::Instant) -> bool {
        self.flush
            || self
                .blocked
                .as_ref()
                .is_some_and(|(k, at, again)| k == key && (*again || now.duration_since(*at) < UNIT_RETRY))
    }
}

/// The UI's mask test and zone trigger into the acquisition settings. The mask unit runs one of them: the mask test
/// wins. AUX OUT pass/fail follows the zone result while the zone trigger runs (0x119c source bit).
#[cfg(any(target_os = "linux", test))]
pub(super) fn hw_merge(
    want: &mut Settings,
    mask: Option<&crate::board::MaskWant>,
    zone: Option<&crate::board::ZoneWant>,
    map: oos_hal::dho::mask::Map,
) {
    want.hwmask = mask.map(|w| oos_hal::dho::seq::HwMask { map, ..hw_mask(w) });
    want.hwzone = zone.filter(|_| mask.is_none()).map(|z| oos_hal::dho::seq::HwZone {
        ch: z.ch,
        zones: z
            .zones
            .iter()
            .take(2)
            .map(|(poly, inside)| oos_hal::dho::zone::Zone { poly: poly.clone(), inside: *inside })
            .collect(),
        clear: z.clear,
        map,
    });
    if let (auxout::AuxOut::PassFail(p), Some(_)) = (&mut want.aux, &want.hwzone) {
        p.source = auxout::Source::Zone;
    }
}
/// The mask test of the acquisition settings from the UI's wish.
#[cfg(any(target_os = "linux", test))]
fn hw_mask(w: &crate::board::MaskWant) -> oos_hal::dho::seq::HwMask {
    oos_hal::dho::seq::HwMask {
        ch: w.ch,
        lo: w.lo.clone(),
        hi: w.hi.clone(),
        stop_on_fail: w.stop,
        clear: w.clear,
        map: oos_hal::dho::mask::Map::default(),
    }
}
