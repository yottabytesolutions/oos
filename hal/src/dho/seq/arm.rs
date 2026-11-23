// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Arm, stop and ring bounds for live and segmented acquisition.

use super::{time_fs, wait_steps, Settings, State, Step};
use crate::dho::{acq, hz, scu};

#[cfg(test)]
mod tests;

impl State {
    /// Arm at frame zero and wait for the first frame to complete.
    pub fn arm_steps(&mut self, s: &Settings, p: &hz::Params) -> Vec<Step> {
        let mut st = self.arm_only_steps(s, p);
        st.extend(wait_steps(p));
        st
    }

    /// Arm at frame zero without waiting, before the host decodes the previous batch.
    pub fn arm_only_steps(&mut self, s: &Settings, p: &hz::Params) -> Vec<Step> {
        // A deep live capture can occupy frame zero beyond the stock segmented-record ring.
        let nf = frames(s, p);
        let mut st = vec![Step::Note(format!("arm (frame 0, {} frame{})", nf, if nf == 1 { "" } else { "s" }))];
        st.extend(scu::arm(s.mask, &mut self.sh, nf).into_iter().map(Step::Wr));
        st
    }

    /// Stops an acquisition armed ahead (`arm_only_steps`) that will not be read, before anything else writes the FPGA: the
    /// stock StopScope (wait up to 10 ms for 0x4034 bit13, then the stop sequence; the same 17 writes every arm starts
    /// with).
    pub fn disarm_steps(&mut self) -> Vec<Step> {
        let mut w = Vec::new();
        scu::stop_sequence(&mut w, &mut self.sh);
        let mut st = vec![
            Step::Note("disarm".into()),
            Step::Poll { reg: 0x4034, bit: scu::status::STOPPED, ms: 10, what: "FSM stopped before the disarm" },
        ];
        st.extend(w.into_iter().map(Step::Wr));
        st
    }

    /// Restarts a hardware average (CLEAR, RUN after STOP, SINGLE, a complete hold average): wait for the FSM to stop,
    /// then the clear pulse (stock StopScope + setAvgClr). Between batches the read-out has already stopped the FSM.
    pub fn avg_clear_steps(&mut self) -> Vec<Step> {
        let mut st = vec![
            Step::Note("average clear".into()),
            Step::Poll { reg: 0x4034, bit: scu::status::STOPPED, ms: 10, what: "FSM stopped before the average clear" },
        ];
        st.extend(acq::avg_clear().into_iter().map(Step::Wr));
        st
    }

    /// Start of a hardware segmented recording: clear the first-trigger stamp, then the record arm of
    /// `record_frames(s, p)` frames with the interval of `s.segments`. Poll the progress with `seg::status`.
    pub fn record_steps(&mut self, s: &Settings, p: &hz::Params) -> Vec<Step> {
        let n = record_frames(s, p);
        let iv = s.segments.map_or(0, |g| scu::interval_ticks(time_fs(g.interval)));
        let mut st = vec![Step::Note(format!("segmented record: {} frames, interval {} x 6.4 ns", n, iv))];
        st.extend(scu::ts_first_clear(&mut self.sh).into_iter().map(Step::Wr));
        st.extend(scu::record_arm(s.mask, &mut self.sh, n, iv).into_iter().map(Step::Wr));
        st
    }
}

/// Frames of a hardware segmented recording: `Segments::n`, at least 1, at most `hz::max_record_frames`; 0 without
/// segments.
pub fn record_frames(s: &Settings, p: &hz::Params) -> u32 {
    s.segments.map_or(0, |g| g.n.clamp(1, hz::max_record_frames(p.seg)))
}

/// Frames of one batch: `Settings::frames`, at least 1, at most the frame ring (`max_frames`); 1 for play-last.
pub fn frames(s: &Settings, p: &hz::Params) -> u32 {
    if !s.playback.is_play4() || s.la.is_some() {
        return 1;
    }
    s.frames.clamp(1, p.max_frames.max(1))
}

pub(super) fn batch_arm(st: &mut State, s: &Settings, p: &hz::Params, armed: bool) -> Vec<Step> {
    if armed {
        wait_steps(p)
    } else {
        st.arm_steps(s, p)
    }
}
