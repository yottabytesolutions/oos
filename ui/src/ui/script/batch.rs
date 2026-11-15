// SPDX-License-Identifier: GPL-2.0-or-later

//! One scripted batch per UI step. The normal UI merges worker maps and draws the newest queued frame.

use crate::board::scripted::{self, Batch, BatchKind, BatchSource, ScriptBytes, ScriptedBatchSource};
use crate::drawpipe::{self, Counters};
use crate::model::RunState;
use crate::ui::Ui;
use crate::wave::Source;
use oos_hal::sync::lock;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

const SCHEDULE: [usize; 6] = [27, 3, 13, 3, 27, 13];

#[derive(Default)]
struct Counts {
    submitted: u64,
    deposited: u64,
    delivered: u64,
}

struct Pending {
    before: (u64, u64, u64),
    frames: u64,
    newest: u64,
    workers: bool,
}

pub struct BatchScript {
    source: ScriptedBatchSource,
    record_only: bool,
    spare: Option<ScriptBytes>,
    id: usize,
    delivered: Arc<AtomicU64>,
    pending: Option<Pending>,
    decoded: Arc<AtomicU64>,
    counts: Counts,
}

impl BatchScript {
    pub fn new(ui: &mut Ui, kind: BatchKind) -> Self {
        ui.src = Source::Hw { shared: Default::default(), seen: 0 };
        // M1 batch captures exclude measurements until M10 refreshes the worker view independently.
        ui.m.meas.clear();
        ui.m.meas2.clear();
        super::super::change::stats_clear(ui);
        Self {
            source: scripted::source(kind, 0x9e3779b97f4a7c15),
            record_only: matches!(kind, BatchKind::Roll | BatchKind::Scan | BatchKind::HardwareAverage),
            spare: None,
            id: 0,
            delivered: Arc::new(AtomicU64::new(0)),
            pending: None,
            decoded: Arc::default(),
            counts: Counts::default(),
        }
    }

    pub fn feed_step(&mut self, ui: &mut Ui) -> Result<(), String> {
        if !ui.m.meas.is_empty() || !ui.m.meas2.is_empty() {
            return Err("scripted batches require measurements off until M10".into());
        }
        if self.pending.is_some() {
            return Err(format!("batch {} still in flight", self.id));
        }
        if ui.m.run != RunState::Run {
            return Ok(());
        }
        ui.relayout();
        let Source::Hw { shared, .. } = &ui.src else { return Err("scripted source replaced".into()) };
        if !lock(shared).queue.is_empty() {
            return Err(format!("batch {} left queued frames", self.id));
        }
        let pipe = drawpipe::pipe(ui.switches()).ok_or("scripted source needs batch pipeline")?;
        pipe.set_view((ui.fast_ok() && !self.record_only).then_some(&ui.m), ui.ph.w, ui.ph.h, ui.switches().interp);
        let (frames, stamp) = capture_plan(ui, self.id);
        let batch = self.source.next_batch(&ui.m, ui.switches(), frames, self.spare.take())?;
        self.id += 1;
        let before = drawpipe::counters().read();
        Counters::add(&drawpipe::counters().acquired, frames as u64);
        let newest = batch.newest(stamp);
        self.pending = Some(Pending { before, frames: frames as u64, newest: newest.seq, workers: pipe.accepting() });
        if pipe.accepting() {
            let (fb, decode, codes) = (batch.frame_bytes(), batch.decode(), batch.codes());
            self.decoded.store(0, Ordering::Relaxed);
            let decoded = self.decoded.clone();
            let decode = Arc::new(move |bytes: &[u8], frame: &mut crate::board::Frame| {
                decoded.fetch_add(1, Ordering::Relaxed);
                decode(bytes, frame);
            });
            let (shared, delivered) = (shared.clone(), self.delivered.clone());
            pipe.feed_with(
                Box::new(batch.into_bytes()),
                fb,
                frames - 1,
                stamp,
                decode,
                codes,
                Box::new(move || {
                    let mut g = lock(&shared);
                    g.frame = newest.clone();
                    g.queue.push_back(newest);
                    delivered.fetch_add(1, Ordering::Relaxed);
                }),
            );
            let returned = drawpipe::host::finish_batch(pipe, self.id as u64)?;
            self.spare = Some(*returned.into_any().downcast::<ScriptBytes>().map_err(|_| "wrong scripted buffer")?);
        } else {
            let mut g = lock(shared);
            for i in 0..frames {
                g.queue.push_back(batch.frame(i, stamp));
            }
            g.frame = newest;
            self.spare = Some(batch.into_bytes());
        }
        Ok(())
    }

    pub fn finish_step(&mut self, ui: &Ui) -> Result<(), String> {
        let Some(p) = self.pending.take() else { return Ok(()) };
        let after = drawpipe::counters().read();
        let Source::Hw { shared, seen } = &ui.src else { return Err("scripted source replaced".into()) };
        let delivered = self.delivered.load(Ordering::Relaxed);
        let expected_delivered = self.counts.delivered + u64::from(p.workers);
        if (after.0 - p.before.0, after.1 - p.before.1, after.2 - p.before.2) != (p.frames, p.frames, 0)
            || *seen != p.newest
            || !lock(shared).queue.is_empty()
            || delivered != expected_delivered
        {
            return Err(format!(
                "batch {} accounting: before {:?}, after {:?}, newest {}/{}, delivered {}/{}",
                self.id, p.before, after, seen, p.newest, delivered, expected_delivered
            ));
        }
        self.counts.submitted += p.frames;
        self.counts.deposited += if p.workers { p.frames - 1 } else { 0 };
        self.counts.delivered = delivered;
        let decoded = self.decoded.load(Ordering::Relaxed);
        let route = if !p.workers {
            "records-decoded"
        } else if decoded == 0 {
            "codes"
        } else if decoded == p.frames - 1 {
            "decoded"
        } else {
            "mixed"
        };
        println!(
            "batch {} submitted {} deposited {} delivered {} dropped {} newest {} route {} decode {}",
            self.id,
            self.counts.submitted,
            self.counts.deposited,
            delivered,
            after.2,
            p.newest,
            if p.workers { "workers" } else { "records" },
            route
        );
        Ok(())
    }
}

fn capture_plan(ui: &Ui, id: usize) -> (usize, u64) {
    let single = ui.m.trig_mode == crate::model::TrigMode::Single;
    // Ui::step arms SINGLE before reading the queue. Supply one post-arm acquisition, as the board would.
    let stamp = ui.m.hw_stamp + u64::from(single && !ui.single_armed);
    (if single { 1 } else { SCHEDULE[id % SCHEDULE.len()] }, stamp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oos_dsp::measure::Meas;

    #[test]
    fn all_batch_kinds_clear_measurements_and_reject_reenabling_them() {
        for kind in [BatchKind::Raw, BatchKind::Codes, BatchKind::LivePairs, BatchKind::DeepPairs] {
            let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
            ui.m.meas.push((Meas::Vpp, 0));
            ui.m.meas2.push((0, 0, 1));
            ui.mstat.push(crate::ui::MStat { n: 7, ..Default::default() });
            let mut batch = BatchScript::new(&mut ui, kind);
            assert!(ui.mstat.is_empty(), "batch measurement removal must use the observed clear seam");
            assert!(ui.m.meas.is_empty());
            assert!(ui.m.meas2.is_empty());
            for run in [RunState::Run, RunState::Stop] {
                ui.m.run = run;
                ui.m.meas.push((Meas::Vpp, 0));
                assert_eq!(
                    batch.feed_step(&mut ui).unwrap_err(),
                    "scripted batches require measurements off until M10"
                );
                ui.m.meas.clear();
                ui.m.meas2.push((0, 0, 1));
                assert_eq!(
                    batch.feed_step(&mut ui).unwrap_err(),
                    "scripted batches require measurements off until M10"
                );
                ui.m.meas2.clear();
                assert_eq!(batch.id, 0, "reject before producing or depositing any frame");
            }
        }
    }
}
