// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! ADC core-offset qualification and matching on the measure unit.

#[cfg(target_os = "linux")]
use super::overlay;

/// Tries per ADC mode and session: a mode whose channels sit far from mid-scale or clip is tried again at the next
/// configurations, at most this often.
#[cfg(target_os = "linux")]
pub(super) const TRIM_TRIES: u8 = 5;

#[cfg(any(target_os = "linux", test))]
/// A core trim waits until the configuration has stood this long: right after the start-up mode switches the front end
/// is still settling (unit round 15: the first try read the kept offsets 4.4 steps apart and a probe-comp signal
/// as quiet; two restarts later the same offsets read 0.29).
pub(super) const TRIM_HOLD: std::time::Duration = std::time::Duration::from_secs(2);

#[cfg(any(target_os = "linux", test))]
/// A core trim waiting for `TRIM_HOLD` after the configuration of ADC mode `mode` with channel set `mask`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TrimPending {
    pub(super) mode: usize,
    pub(super) mask: u32,
    pub(super) since: std::time::Instant,
}

#[cfg(any(target_os = "linux", test))]
impl TrimPending {
    pub(super) fn due(&self, now: std::time::Instant) -> bool {
        now.saturating_duration_since(self.since) >= TRIM_HOLD
    }
}

/// The unit side of `coretrim`: measure unit runs and ADC core offset writes.
#[cfg(target_os = "linux")]
struct UnitTrim<'a> {
    bar: &'a mut oos_hal::dho::bus::Bar1,
    adc: &'a mut oos_hal::dho::adc::Adc,
}

#[cfg(target_os = "linux")]
impl oos_hal::dho::coretrim::TrimHw for UnitTrim<'_> {
    fn slots(&mut self) -> Option<[oos_hal::dho::levels::Slot; 8]> {
        oos_hal::dho::levels::run(self.bar, &mut |us| std::thread::sleep(std::time::Duration::from_micros(us as u64)))
    }
    fn wait_ms(&mut self, ms: u32) {
        std::thread::sleep(std::time::Duration::from_millis(ms as u64));
    }
    fn set_offset(&mut self, core: usize, o: i32) {
        use oos_hal::dho::bus::Bus;
        for w in self.adc.core_offset(core, o) {
            std::thread::sleep(std::time::Duration::from_micros(w.pre_us as u64));
            self.bar.wr(w.reg, w.val);
            std::thread::sleep(std::time::Duration::from_micros(w.post_us as u64));
        }
    }
}

/// Matches the core offsets of the active ADC mode (`coretrim`), keeps them in `cal` and in `overlay`, and saves the
/// overlay to OOS's own cal directory when they changed (never a Rigol file, never cal_adc.hex). True when the mode is
/// done (matched, or nothing to change); false to try again at a later configuration.
#[cfg(target_os = "linux")]
pub(super) fn core_trim(
    bar: &mut oos_hal::dho::bus::Bar1,
    state: &mut oos_hal::dho::seq::State,
    cal: &mut oos_hal::dho::adc::CalAdc,
    overlay: &mut oos_hal::dho::coretrim::Overlay,
    mask: u32,
) -> bool {
    use oos_hal::dho::{calfile, coretrim};
    let mode = state.mode;
    let cur = cal.offsets(mode);
    let (groups, lsb) = coretrim::plan(mask, &state.gain);
    let mut hw = UnitTrim { bar, adc: &mut state.adc };
    // the registers may still hold what the cold boot or the stock system wrote: start from `cal`'s values
    for (n, &o) in cur.iter().enumerate() {
        coretrim::TrimHw::set_offset(&mut hw, n, o);
    }
    coretrim::TrimHw::wait_ms(&mut hw, coretrim::TRIM_SETTLE_MS);
    // gates first: a mode whose input carries a signal is left alone without a nudge or a log line, and tried again
    // at a later configuration
    let g = match coretrim::gate(&mut hw, &groups, &lsb) {
        Ok(g) if g.matched != 0 => g,
        Ok(_) => return false,
        Err(e) => {
            eprintln!("oos-ui hw: core offsets of ADC mode {}: {}", mode, e);
            return false;
        }
    };
    let order = match coretrim::order(&mut hw, &cur) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("oos-ui hw: core offsets of ADC mode {}: core order not found: {}", mode, e);
            return false;
        }
    };
    match coretrim::trim_gated(&mut hw, &groups, &cur, &order, &lsb, &g) {
        Ok(t) => {
            eprintln!("oos-ui hw: core offsets of ADC mode {}: {:?} -> {:?}, order {:?}, {:.2} steps left (kept {:.2}), matched {:#x} skipped {:#x}", mode, cur, t.offsets, order, t.residual, t.kept, t.matched, t.skipped);
            if t.offsets != cur {
                // this session uses the result; the file only takes a move beyond the per-boot scatter
                cal.set_offsets(mode, &t.offsets);
                if !coretrim::worth_saving(&cur, &t.offsets) {
                    eprintln!("oos-ui hw: core offsets of ADC mode {}: within {} register steps of the kept values, not saved", mode, coretrim::SAVE_MIN_STEPS);
                    return t.matched != 0 && t.skipped == 0;
                }
                overlay.set(mode, &t.offsets);
                if let Err(e) =
                    calfile::save(calfile::OOS_CAL_DIR, coretrim::OVERLAY_FILE, &overlay.to_bytes(), calfile::now())
                {
                    eprintln!("{}", overlay::save_error(&e));
                }
            }
            t.matched != 0 && t.skipped == 0
        }
        Err(e) => {
            // the registers hold the last round's values; the stored ones go back
            for (n, &o) in cur.iter().enumerate() {
                coretrim::TrimHw::set_offset(&mut hw, n, o);
            }
            eprintln!("oos-ui hw: core offsets of ADC mode {}: {}, order {:?}", mode, e, order);
            false
        }
    }
}
