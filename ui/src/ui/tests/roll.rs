// SPDX-License-Identifier: GPL-2.0-or-later

//! Regression for roll snapshots, STOP and fresh rings after rearm.

// The legacy board boundary scanner also needs an explicit test-only block in this file.
#[cfg(test)]
mod tests {
    use crate::board::dho::roll::{publish, roll_run};
    use crate::ui::*;
    use std::collections::VecDeque;
    use std::sync::Arc;

    const KEEP: usize = 15_625;

    struct RollFixture {
        ui: Ui,
        shared: crate::board::HwShared,
        ring: [VecDeque<f32>; NCH],
        ring_la: VecDeque<u16>,
        params: oos_hal::dho::roll::Params,
        rolling: Option<oos_hal::dho::roll::Params>,
        seq: u64,
    }

    impl RollFixture {
        fn new() -> Self {
            let shared: crate::board::HwShared = Default::default();
            let mut ui = Ui::new(Source::Hw { shared: shared.clone(), seen: 0 }, host::Host::fixture());
            ui.m.run = RunState::Run;
            ui.m.trig_mode = TrigMode::Single;
            ui.m.auto_roll = true;
            ui.m.tscale = tscales().iter().position(|&t| t == 0.5).unwrap();
            for ch in &mut ui.m.ch {
                ch.on = true;
            }
            ui.step(0.02);
            assert_eq!(shared.lock().unwrap().slow, crate::board::Slow::Roll);
            let s = crate::board::dho::hw::settings(&ui.m, false, ui.switches());
            let tdiv_fs = oos_hal::dho::seq::tdiv_fs(s.tdiv);
            let p = oos_hal::dho::hz::prep(s.mask, tdiv_fs, s.depth, 0);
            let params = oos_hal::dho::roll::params(s.mask, tdiv_fs, s.depth, &p);
            Self {
                ui,
                shared,
                ring: Default::default(),
                ring_la: VecDeque::new(),
                params,
                rolling: Some(params),
                seq: 0,
            }
        }

        // Same control and publication helpers as the hardware loop; no board or stream is opened.
        fn tick(&mut self, chunks: usize, value: f32) {
            let mut g = self.shared.lock().unwrap();
            if !roll_run(&mut g, &mut self.rolling, &mut self.ring, &mut self.ring_la) {
                assert!(self.rolling.is_none() && self.ring.iter().all(|r| r.is_empty()) && self.ring_la.is_empty());
                return;
            }
            self.rolling = Some(self.params);
            for ch in &mut self.ring {
                ch.extend(std::iter::repeat_n(value, chunks * 25));
                while ch.len() > KEEP {
                    ch.pop_front();
                }
            }
            self.ring_la.extend(std::iter::repeat_n(value as u16, chunks * 25));
            while self.ring_la.len() > KEEP {
                self.ring_la.pop_front();
            }
            self.seq += 1;
            let len = self.ring[0].len();
            let stamp = g.acquisition.identity().epoch();
            let cfg = g.acquisition.identity().gen();
            publish(
                &mut g,
                crate::board::Frame {
                    seq: self.seq,
                    dt: 320e-6,
                    pre: (len as f64 - 1.0 - 2.5 / 320e-6).max(0.0) as usize,
                    ch: self.ring.clone().map(|r| r.into_iter().collect()),
                    roll: true,
                    envelope: true,
                    partial: len < KEEP,
                    stamp,
                    cfg,
                    la: Some(la::LaRec {
                        words: Arc::new(self.ring_la.iter().copied().collect()),
                        t0: 0.0,
                        dt: 320e-6,
                    }),
                    ..Default::default()
                },
                false,
            );
        }

        fn fill_and_stop(&mut self) {
            self.tick(1, 1.0);
            self.ui.step(0.02);
            assert_eq!(self.ui.m.run, RunState::Run, "partial SINGLE keeps running");
            // More than one full screen, with the UI not consuming every picture.
            for _ in 0..626 {
                self.tick(1, 1.0);
            }
            for i in 0..300 {
                self.tick(i % 2, 1.0);
                let g = self.shared.lock().unwrap();
                let bytes = queued_bytes(&g);
                assert!(
                    bytes <= KEEP * NCH * std::mem::size_of::<f32>() && g.queue.len() <= crate::board::QUEUE_MAX,
                    "running roll queue: {} bytes in {} snapshots",
                    bytes,
                    g.queue.len()
                );
                assert_eq!(g.queue.len(), 1);
                assert_eq!(g.queue.back().unwrap().seq, g.frame.seq);
            }
            self.ui.step(0.02);
            assert_eq!(self.ui.m.run, RunState::Stop, "complete SINGLE stops");
            self.ui.step(0.02);
            assert!(self.shared.lock().unwrap().hold);
        }

        fn assert_fresh(&self, value: f32) {
            let g = self.shared.lock().unwrap();
            assert!(!g.rearm && g.frame.partial);
            assert!(g.frame.ch.iter().all(|ch| ch.len() == 25 && ch.iter().all(|&v| v == value)));
            assert_eq!(g.frame.la.as_ref().unwrap().words.as_slice(), [value as u16; 25]);
        }
    }

    fn acquired() -> u64 {
        crate::drawpipe::counters().read().0
    }

    fn queued_bytes(g: &crate::board::Shared) -> usize {
        g.queue.iter().flat_map(|f| &f.ch).map(|ch| ch.len() * std::mem::size_of::<f32>()).sum()
    }

    // Break it catches: complete roll snapshots accumulating after SINGLE, including empty stream ticks;
    // STOP bypassed by the roll loop, old samples on rearm, and roll missing from the acquired counter.
    #[test]
    fn roll_single_bounds_snapshots_holds_and_rearms() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let mut f = RollFixture::new();
        let before = acquired();
        f.fill_and_stop();
        let stopped = f.shared.lock().unwrap().frame.seq;
        let counted = acquired();
        for i in 0..400 {
            f.tick(i % 2, 2.0); // half the ticks have no new chunks
            f.ui.step(0.02);
        }
        let g = f.shared.lock().unwrap();
        assert!(queued_bytes(&g) <= KEEP * NCH * std::mem::size_of::<f32>());
        assert!(g.queue.len() <= 1 && g.queue.len() <= crate::board::QUEUE_MAX);
        assert_eq!(g.frame.seq, stopped, "held roll produced another snapshot");
        drop(g);
        let held_count = acquired();
        assert_eq!(f.ui.last[0].as_ref().and_then(|r| r.raw.as_ref()).map(|r| r.frame.seq), Some(stopped));
        assert_eq!(counted - before, 927, "every roll publication is acquired");
        assert_eq!(held_count, counted, "held ticks acquire nothing");
        // RUN and a rearm each start with only newly arriving analog and LA samples.
        f.ui.m.run = RunState::Run;
        f.ui.step(0.02);
        f.tick(1, 3.0);
        f.assert_fresh(3.0);
        f.shared.lock().unwrap().rearm = true;
        f.tick(1, 4.0);
        f.assert_fresh(4.0);
        assert_eq!(acquired() - held_count, 2, "resume and rearm each acquire one fresh picture");
    }
}
