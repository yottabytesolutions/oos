// SPDX-License-Identifier: GPL-2.0-or-later

use super::{hw_record, Model, Record, Source};
use crate::board::{PictureEligibility, QueuedFrame};
use crate::drawpipe::{self, TapRead};

impl Source {
    /// Hardware source: takes the newest `max` queued frames but the newest one (the fast display path draws them;
    /// the newest stays for `acquire`, which makes the full record for measurements, decoding and history). Older
    /// ones are dropped and counted.
    pub fn drain_newest(&mut self, max: usize) -> Vec<std::sync::Arc<crate::board::Frame>> {
        if let Source::Hw { shared, .. } = self {
            let mut g = crate::perfguard::deep::ui_lock(shared);
            // the thread decodes no more than this of a batch (the record takes one more)
            g.take = max + 1;
            let id = g.acquisition.identity();
            let before = g.queue.len();
            g.queue.retain(|f| (f.cfg, f.stamp) == (id.gen(), id.epoch()));
            crate::drawpipe::Counters::add(&crate::drawpipe::counters().dropped, (before - g.queue.len()) as u64);
            crate::drawpipe::Counters::add(
                &crate::drawpipe::counters().losses.queue_pop,
                (before - g.queue.len()) as u64,
            );
            // Leave interleaved fallbacks for ordered reader intake.
            if g.queue.len() > 1 && drawpipe::pipe(drawpipe::Access::Existing).is_some_and(|p| p.has_waveform(id)) {
                return Vec::new();
            }
            let prefix = g.queue.len().saturating_sub(1);
            let skip = prefix.saturating_sub(max);
            crate::drawpipe::counters().lose(crate::drawpipe::Lost::Budget, skip as u64);
            g.dropped += skip as u64;
            return g.queue.drain(..prefix).skip(skip).collect();
        }
        Vec::new()
    }

    pub(crate) fn acquire_hw(
        &mut self,
        m: &Model,
        take_tap: impl FnOnce(oos_core::change::Identity, Option<u64>) -> TapRead<QueuedFrame>,
    ) -> Option<(Record, PictureEligibility, bool)> {
        let Source::Hw { shared, seen } = self else { return None };
        let mut g = crate::perfguard::deep::ui_lock(shared);
        let id = g.acquisition.identity();
        while g.queue.pop_front_if(|f| (f.cfg, f.stamp) != (id.gen(), id.epoch())).is_some() {
            drawpipe::Counters::add(&drawpipe::counters().losses.queue_pop, 1);
            drawpipe::Counters::add(&drawpipe::counters().dropped, 1);
        }
        // Shared stays locked across the slot take. A newer fallback cannot be inserted
        // between comparing the FIFO heads and selecting the next reader record.
        let (f, tap) = match take_tap(id, g.queue.front().map(|f| f.seq)) {
            TapRead::Ready(tap) => {
                if (g.frame.cfg, g.frame.stamp) != (tap.cfg, tap.stamp) || g.frame.seq <= tap.seq {
                    g.frame = (*tap).clone();
                }
                (tap, true)
            }
            TapRead::Pending | TapRead::Stale => return None,
            TapRead::Empty => match g.queue.pop_front() {
                Some(frame) => (QueuedFrame::new(frame, PictureEligibility::Drawable), false),
                None => {
                    if g.halted {
                        g.halted = false;
                        g.rearm = true;
                    }
                    return None;
                }
            },
        };
        *seen = f.seq;
        drop(g);
        let eligibility = f.eligibility();
        let deep = crate::deepmeas::deep_raw_frame(&f);
        let t = std::time::Instant::now();
        let r = f.read(|frame| hw_record(m, frame));
        if deep {
            crate::deepmeas::cost::set(&crate::deepmeas::cost::RECORD_US, t.elapsed());
        }
        Some((r, eligibility, tap))
    }
}
