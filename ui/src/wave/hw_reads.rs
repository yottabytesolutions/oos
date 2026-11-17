// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The hardware source's readings and requests through the acquisition thread's `board::Shared`: counter, DVM,
//! mask and zone units, slots, board status, and the segment recorder. Other sources answer None or nothing.

use super::{Record, SegView, Source};
use crate::model::{Model, HDIV};

impl Source {
    /// Hardware source: hands the counter set-up the UI wants to the acquisition thread (None: off) and returns the
    /// latest reading; None off the hardware (the counter is then worked out from the records).
    pub fn hw_counter(&self, want: Option<crate::board::CounterWant>) -> Option<Option<crate::board::CounterRead>> {
        if let Source::Hw { shared, .. } = self {
            let mut g = crate::perfguard::deep::ui_lock(shared);
            g.counter_want = want;
            return Some(g.counter);
        }
        None
    }

    /// Hardware source: why the mask/zone unit is off for the current settings (`Shared::unit_blocked`).
    pub fn hw_unit_blocked(&self) -> Option<String> {
        if let Source::Hw { shared, .. } = self {
            return crate::perfguard::deep::ui_lock(shared).unit_blocked.clone();
        }
        None
    }

    /// Hardware source: the mask table last loaded (`Shared::mask_table`).
    pub fn hw_mask_table(&self) -> Option<std::sync::Arc<Vec<[u8; 4]>>> {
        if let Source::Hw { shared, .. } = self {
            return crate::perfguard::deep::ui_lock(shared).mask_table.clone();
        }
        None
    }

    /// Latest zone trigger reading; acquisition publication owns its request.
    pub fn hw_zone(&self) -> Option<Option<crate::board::ZoneRead>> {
        if let Source::Hw { shared, .. } = self {
            return Some(crate::perfguard::deep::ui_lock(shared).zone);
        }
        None
    }

    /// Latest hardware mask counters; acquisition publication owns its request.
    pub fn hw_mask(&self) -> Option<Option<crate::board::MaskRead>> {
        if let Source::Hw { shared, .. } = self {
            return Some(crate::perfguard::deep::ui_lock(shared).mask);
        }
        None
    }

    /// Hardware source: hands the DVM set-up the UI wants to the acquisition thread (None: off) and returns the latest
    /// reading; None off the hardware.
    pub fn hw_dvm(&self, want: Option<crate::board::DvmWant>) -> Option<Option<crate::board::DvmRead>> {
        if let Source::Hw { shared, .. } = self {
            let mut g = crate::perfguard::deep::ui_lock(shared);
            g.dvm_want = want;
            return Some(g.dvm);
        }
        None
    }

    /// Hardware source: asks for one raw measure-unit run (`Shared::slots_want`) and returns the latest answer; None
    /// for other sources.
    pub fn hw_slots(&self, want: u64) -> Option<Option<crate::board::SlotDump>> {
        if let Source::Hw { shared, .. } = self {
            let mut g = crate::perfguard::deep::ui_lock(shared);
            g.slots_want = Some(want);
            return Some(g.slots);
        }
        None
    }

    /// Hardware source: the latest trigger status of a batch (`Shared::trig_fired`).
    pub fn hw_trig_fired(&self) -> Option<(u64, Option<bool>)> {
        if let Source::Hw { shared, .. } = self {
            return crate::perfguard::deep::ui_lock(shared).trig_fired;
        }
        None
    }

    /// Hardware source: FPGA version (0 = not reached) and the latest XADC reading (die degC, VCCINT V).
    pub fn hw_board(&self) -> Option<(u32, Option<(f32, f32)>)> {
        if let Source::Hw { shared, .. } = self {
            let g = crate::perfguard::deep::ui_lock(shared);
            return Some((g.fpga_version, g.xadc));
        }
        None
    }

    /// Hardware segments: tells the thread the recording number and whether the acquisition runs; returns phase,
    /// segments asked for, recorded and read back, and whether frames are still queued. None without the hardware
    /// source.
    pub fn seg_sync(&self, run: u64, running: bool) -> Option<SegView> {
        let Source::Hw { shared, .. } = self else { return None };
        let mut g = crate::perfguard::deep::ui_lock(shared);
        (g.seg.run, g.seg.running) = (run, running);
        let current = g.seg.done_run == run;
        if !current || g.seg.phase == crate::board::SegPhase::Idle {
            (g.seg.want, g.seg.shown) = (None, None);
        }
        Some(SegView {
            phase: if current { g.seg.phase } else { crate::board::SegPhase::Idle },
            total: g.seg.total,
            recorded: g.seg.recorded,
            read: g.seg.read,
            queued: !g.queue.is_empty(),
        })
    }

    /// Hardware segments: the trigger times read back so far (seconds since the first trigger).
    pub fn seg_times(&self) -> Vec<Option<f64>> {
        match self {
            Source::Hw { shared, .. } => crate::perfguard::deep::ui_lock(shared).seg.times.clone(),
            _ => Vec::new(),
        }
    }

    /// Hardware segments: asks the thread to play segment `k`; `seg_shown` delivers it.
    pub fn seg_request(&self, k: u32) {
        if let Source::Hw { shared, .. } = self {
            let mut g = crate::perfguard::deep::ui_lock(shared);
            (g.seg.want, g.seg.shown) = (Some(k), None);
        }
    }

    /// Hardware segments: the segment asked for with `seg_request`, as a record for the screen.
    pub fn seg_shown(&self, m: &Model) -> Option<(u32, Record)> {
        let Source::Hw { shared, .. } = self else { return None };
        let mut g = crate::perfguard::deep::ui_lock(shared);
        let (k, f) = g.seg.shown.take()?;
        let id = g.acquisition.identity();
        if (f.cfg, f.stamp) != (id.gen(), id.epoch()) {
            crate::drawpipe::Counters::add(&crate::drawpipe::counters().losses.queue_pop, 1);
            crate::drawpipe::Counters::add(&crate::drawpipe::counters().dropped, 1);
            return None;
        }
        drop(g);
        let window = m.time_per_div() as f64 * HDIV as f64;
        Some((k, crate::board::to_record(&std::sync::Arc::new(f), m, 3600, window, m.delay as f64 - window / 2.0)))
    }
}
