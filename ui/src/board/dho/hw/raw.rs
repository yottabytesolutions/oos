// SPDX-License-Identifier: GPL-2.0-or-later

//! Raw reads decoded off the shared lock (upstream 3930159f).

use super::{batch, decode_into_with};
use crate::board::Frame;
use oos_hal::dho::{hz, seq::Settings};
use std::time::{Duration, Instant};

// Three triggers per 200 ms (two geometry guards and one reference) are 15/s.
// Leave peak below 75/s (20% overhead) and re-enter at 100/s (15% overhead).
const LEAVE_RATE: u32 = 75;
const ENTER_RATE: u32 = 100;

#[derive(Default)]
pub(super) struct ReferenceCycle {
    last: Option<((u64, u64), Instant)>,
    rate: Option<TriggerRate>,
    full_applied: bool,
}

struct TriggerRate {
    pair: (u64, u64),
    qualifying: u8,
    steady: bool,
    rejected: bool,
}

impl ReferenceCycle {
    pub(super) fn reset(&mut self) {
        self.last = None;
        self.full_applied = false;
    }

    pub(super) fn track(&mut self, pair: (u64, u64)) {
        if self.rate.as_ref().is_none_or(|rate| rate.pair != pair) {
            self.rate = Some(TriggerRate { pair, qualifying: 0, steady: false, rejected: false });
        }
    }

    pub(super) fn can_reduce(&self, pair: (u64, u64)) -> bool {
        self.rate.as_ref().is_some_and(|rate| rate.pair == pair && rate.steady && !rate.rejected)
    }

    pub(super) fn reject(&mut self, pair: (u64, u64)) {
        self.track(pair);
        if let Some(rate) = self.rate.as_mut() {
            rate.rejected = true;
        }
    }

    pub(super) fn observe(&mut self, pair: (u64, u64), records: u32, elapsed: Duration) {
        let Some(rate) = self.rate.as_mut().filter(|rate| rate.pair == pair && !rate.rejected) else { return };
        let meets = |minimum| elapsed.saturating_mul(minimum) <= Duration::from_secs(u64::from(records));
        if rate.steady {
            if !meets(LEAVE_RATE) {
                rate.steady = false;
                rate.qualifying = 0;
            }
        } else if meets(ENTER_RATE) {
            rate.qualifying += 1;
            rate.steady = rate.qualifying >= 2;
        } else {
            rate.qualifying = 0;
        }
    }

    pub(super) fn needs_switch(&self, full: bool) -> bool {
        self.full_applied != full
    }

    pub(super) fn applied(&mut self, full: bool) {
        self.full_applied = full;
    }

    pub(super) fn remaining(&self, pair: (u64, u64), now: Instant) -> Duration {
        self.last.filter(|(captured, _)| *captured == pair).map_or(Duration::ZERO, |(_, at)| {
            crate::compress::RAW_PERIOD.saturating_sub(now.saturating_duration_since(at))
        })
    }

    pub(super) fn completed(&mut self, pair: (u64, u64), now: Instant) {
        self.last = Some((pair, now));
    }
}

pub(super) fn reference_settings(s: &Settings) -> Settings {
    Settings { mode: oos_hal::dho::acq::Mode::Normal, window: None, ..oos_hal::dho::seq::raw_settings(s) }
}

pub(super) fn is_reduced(s: &Settings) -> bool {
    matches!(s.mode, oos_hal::dho::acq::Mode::Reduced(_)) && s.mode.is_peak()
}

/// The column kernel indexes stored words and groups consecutive words into pairs.
/// Each word takes one programmed sample period. The trigger is on that same word grid.
pub(super) fn peak_word_axis(frame: &mut Frame, s: &Settings, p: &hz::Params) {
    let (_, start) = oos_hal::dho::seq::window(s, p);
    frame.dt = 1.0 / p.rate as f64;
    frame.pre = (p.pre - start) as usize;
    frame.fixed_ti = Some(frame.pre as f64);
}

/// Decode a raw replay using the current batch’s ADC matching decision.
pub(super) fn decode_raw(
    bytes: &[u8],
    (s, p): (&Settings, &hz::Params),
    probe: crate::board::dho::switches::HeaderProbe,
    cores: Option<&mut crate::board::dho::cores::Cores>,
    charge: Option<&std::sync::Arc<crate::drawpipe::TapReservation>>,
) -> Option<batch::DecodedRecord> {
    let mut frame = if let Some(charge) = charge {
        let storage = batch::storage(s, bytes.len())?;
        if !charge.covers(&storage) {
            return None;
        }
        storage
    } else {
        Frame::default()
    };
    let uncorrected = decode_into_with(bytes, s, p, 0, &mut frame, probe, cores);
    if let Some(charge) = charge {
        frame.basic.retain(charge.clone());
    }
    Some((frame, uncorrected))
}

pub(super) fn replay_due(
    count: u32,
    live: bool,
    last: &mut Option<std::time::Instant>,
    clock: &impl oos_hal::dho::seq::time::Clock,
) -> bool {
    if live && count > 0 && last.is_none_or(|t| clock.now().saturating_duration_since(t) >= crate::compress::RAW_PERIOD)
    {
        *last = Some(clock.now());
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_rate_hysteresis_and_timeout_are_scoped_to_the_identity() {
        let mut cycle = ReferenceCycle::default();
        let pair = (5, 2);
        cycle.track(pair);
        let observe = |cycle: &mut ReferenceCycle, rate| {
            cycle.observe(pair, rate, Duration::from_secs(1));
            cycle.can_reduce(pair)
        };
        assert!(!observe(&mut cycle, 99));
        assert!(!observe(&mut cycle, 100));
        assert!(observe(&mut cycle, 100));
        assert!(observe(&mut cycle, 75));
        assert!(!observe(&mut cycle, 74));
        assert!(!observe(&mut cycle, 99));
        assert!(!observe(&mut cycle, 100));
        assert!(observe(&mut cycle, 100));
        cycle.reject(pair);
        assert!(!observe(&mut cycle, 500));
        let next = (5, 3);
        cycle.track(next);
        for _ in 0..2 {
            cycle.observe(next, 100, Duration::from_secs(1));
        }
        assert!(cycle.can_reduce(next));
    }
}
