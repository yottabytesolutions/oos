// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum OperationState {
    Pending,
    Complete,
    Cancelled,
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum OperationToken {
    Auto(u64),
    Sim(u64, u64),
    Single(u64),
}
/// Exact finite jobs at the barrier. Later captures never complete a cancelled job.
#[derive(Clone, Debug)]
pub(crate) struct Operations {
    epoch: u64,
    auto: Option<u64>,
    sim: Option<(u64, u64)>,
    single: Option<(u64, bool)>,
}
impl Operations {
    pub(crate) fn state(&self, ui: &Ui) -> OperationState {
        use OperationState::*;
        if self.epoch != ui.workflow_epoch {
            return Cancelled;
        }
        let now = ui.scpi_operations();
        let mut pending = false;
        for (token, active) in [
            (self.auto.map(OperationToken::Auto), self.auto.is_some() && self.auto == now.auto),
            (self.sim.map(|(epoch, id)| OperationToken::Sim(epoch, id)), self.sim.is_some() && self.sim == now.sim),
            (
                self.single.map(|(arm, _)| OperationToken::Single(arm)),
                self.single.is_some() && self.single == now.single,
            ),
        ] {
            if let Some(token) = token {
                if !ui.scpi_completed.contains(&token) {
                    if active {
                        pending = true;
                    } else {
                        return Cancelled;
                    }
                }
            }
        }
        if pending {
            Pending
        } else {
            Complete
        }
    }
}
impl Ui {
    /// The samples a `:WAVeform` read uses. `memory` is RAW, or MAXimum while stopped: the deep record when the
    /// acquisition kept one, otherwise the screen record.
    fn channel_view(&self, c: usize, memory: bool) -> Option<(&[f32], f64, f64)> {
        let rec = if memory { self.raw_read_record() } else { self.displayed_reader_record() }?;
        if memory {
            if let Some(raw) = rec.raw.as_ref().filter(|r| !r.frame.envelope) {
                if let Some(v) = raw.frame.ch.get(c) {
                    if !v.is_empty() && raw.frame.dt.is_finite() && raw.frame.dt > 0.0 {
                        return Some((v.as_slice(), raw.frame.dt, raw.t0 - raw.i0 as f64 * raw.frame.dt));
                    }
                }
            }
        }
        let v = rec.ch.get(c)?;
        (!v.is_empty()).then_some((v.as_slice(), rec.dt, rec.t0))
    }

    /// How many points `channel_page` can walk. Zero when that source has no record.
    pub fn channel_points(&self, c: usize, memory: bool) -> usize {
        self.channel_view(c, memory).map(|(v, _, _)| v.len()).unwrap_or(0)
    }

    /// `(dt, t0)` of that same record. None when it is empty.
    pub fn channel_origin(&self, c: usize, memory: bool) -> Option<(f64, f64)> {
        self.channel_view(c, memory).map(|(_, dt, t0)| (dt, t0))
    }

    /// One page of a channel, 1-based inclusive, the same numbering as `:WAVeform:STARt` and `:WAVeform:STOP`.
    /// `stop == 0` reads through the last point. The time is the first sample of the page.
    pub fn channel_page(&self, c: usize, memory: bool, start: u64, stop: u64) -> Option<(Vec<f32>, f64, f64)> {
        let (v, dt, t0) = self.channel_view(c, memory)?;
        Some(copy_page(v, dt, t0, start, stop))
    }

    /// Borrow samples and timing together so points, metadata and data select the same record.
    pub(crate) fn scpi_wave_view(&self, source: WaveId, memory: bool, raw: bool) -> Option<(&[f32], f64, f64)> {
        match source {
            WaveId::Ch(c) => self.channel_view(c, memory && (raw || !self.readers_use_average())),
            WaveId::Ref(slot) => {
                let record = self.refs.get(slot)?.as_ref()?;
                (record.v.len() >= 2 && record.dt > 0.0).then_some((record.v.as_slice(), record.dt, record.t0))
            }
        }
    }

    pub(super) fn scpi_finish_operation(&mut self, token: OperationToken) {
        // Receipts are metadata only, bounded independently of waveform history.
        // An expired fence is interrupted rather than falsely completed.
        if self.scpi_completed.back() != Some(&token) {
            if self.scpi_completed.len() == 128 {
                self.scpi_completed.pop_front();
            }
            self.scpi_completed.push_back(token);
        }
    }
    pub(crate) fn scpi_arm_operation(&mut self) {
        self.operation_serial = self.operation_serial.wrapping_add(1);
        self.single_operation = self.operation_serial;
    }
    pub(crate) fn scpi_operations(&self) -> Operations {
        Operations {
            epoch: self.workflow_epoch,
            auto: self.auto_operation(),
            sim: self.sim_operation(),
            single: (self.m.trig_mode == TrigMode::Single && self.m.run == RunState::Run)
                .then(|| (self.single_operation, self.record_mode())),
        }
    }
}
