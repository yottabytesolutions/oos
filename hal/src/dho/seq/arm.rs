// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Arm, stop and ring bounds for live and segmented acquisition.

use super::{time::Clock, time_fs, wait::poll_checked, wait_steps, Settings, State, Step};
use crate::dho::{acq, bus::Bus, hz, scu, Write};
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// Hardware evidence from a cancelled capture, sampled before the play-info clear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureStop {
    Unknown,
    NoCompletedRecord,
    Recorded,
}

/// Status sampled after the stop pulse, before clearing play info.
#[derive(Debug)]
pub struct CaptureDisarm {
    fsm: Option<u32>,
    play_info: Option<u32>,
    stopped_after: Option<Duration>,
    error: Option<String>,
}

impl CaptureDisarm {
    pub fn outcome(&self) -> CaptureStop {
        if !self.status_available() || self.stopped_after.is_none() {
            CaptureStop::Unknown
        } else if self.play_info.is_some_and(|value| value & scu::PLAYINFO_DONE != 0) {
            CaptureStop::Recorded
        } else {
            CaptureStop::NoCompletedRecord
        }
    }

    fn status_available(&self) -> bool {
        self.error.is_none()
            && self.fsm.is_some_and(|value| value != u32::MAX)
            && self.play_info.is_some_and(|value| value != u32::MAX)
    }

    #[cfg(test)]
    fn fsm(&self) -> Option<u32> {
        self.fsm
    }

    #[cfg(test)]
    fn play_info(&self) -> Option<u32> {
        self.play_info
    }

    #[cfg(test)]
    fn stopped_after(&self) -> Option<Duration> {
        self.stopped_after
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

fn disarm_poll() -> Step {
    Step::Poll { reg: 0x4034, bit: scu::status::STOPPED, ms: 10, what: "FSM stopped before the disarm" }
}

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
        let mut st = vec![Step::Note("disarm".into()), disarm_poll()];
        st.extend(self.disarm_writes().into_iter().map(Step::Wr));
        st
    }

    fn disarm_writes(&mut self) -> Vec<Write> {
        let mut writes = Vec::new();
        scu::stop_sequence(&mut writes, &mut self.sh);
        writes
    }

    /// Always execute the stop writes. Sample STOPPED and DONE before clearing play info.
    pub fn disarm_capture(&mut self, bar: &mut impl Bus, clock: &impl Clock) -> CaptureDisarm {
        let before = poll_checked(&disarm_poll(), bar, clock).err();
        let writes = self.disarm_writes();
        let (pulse, finish) = writes.split_at(scu::STOP_PULSE_WRITES);
        let (setup, edge) = pulse.split_at(pulse.len() - 2);
        for &(reg, value) in setup {
            bar.wr(reg, value);
        }
        let pulsed_at = clock.now();
        bar.apply(edge);
        let (fsm, stopped_after) = sample_stopped(bar, clock, pulsed_at);
        let play_info = bar.try_rd(0x4080);
        bar.apply(finish);
        let (fsm, fsm_error) = sample(fsm, 0x4034);
        let (play_info, play_error) = sample(play_info, 0x4080);
        CaptureDisarm { fsm, play_info, stopped_after, error: before.or(fsm_error).or(play_error) }
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

fn sample_stopped(
    bar: &mut impl Bus,
    clock: &impl Clock,
    pulsed_at: Instant,
) -> (std::io::Result<u32>, Option<Duration>) {
    // Observe late assertions without adding more than 500 us of polling to a canceled capture.
    const LIMIT: Duration = Duration::from_micros(500);
    const INTERVAL: Duration = Duration::from_micros(50);
    loop {
        let fsm = bar.try_rd(0x4034);
        let elapsed = clock.now().saturating_duration_since(pulsed_at);
        if fsm.as_ref().is_ok_and(|&value| value != u32::MAX && value & scu::status::STOPPED != 0) {
            return (fsm, (elapsed <= LIMIT).then_some(elapsed));
        }
        if fsm.as_ref().is_err() || fsm.as_ref().is_ok_and(|&value| value == u32::MAX) || elapsed >= LIMIT {
            return (fsm, None);
        }
        clock.wait(INTERVAL.min(LIMIT - elapsed));
    }
}

fn sample(read: std::io::Result<u32>, reg: u32) -> (Option<u32>, Option<String>) {
    match read {
        Ok(value) => (Some(value), None),
        Err(error) => (None, Some(format!("read {reg:#06x}: {error}"))),
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
