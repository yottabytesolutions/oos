// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Model, Source};
use crate::board::{
    identity::{ReplayLoss, StopView},
    Acquired, Frame, PictureEligibility, QueuedFrame,
};
use crate::drawpipe::{self, TapRead};

impl Source {
    /// Stage the hold and final-record request under Shared. The caller publishes the wake.
    pub(crate) fn stage_hold(g: &mut crate::board::Shared, hold: bool, req: u64) -> bool {
        if g.hold != hold {
            if let Some(pipe) = drawpipe::pipe() {
                pipe.stage_tap_hold(hold);
            }
        }
        let changed = g.hold != hold || g.final_req != req;
        (g.hold, g.final_req) = (hold, req);
        changed
    }

    /// Ask the acquisition thread for one force-trigger pulse. Other sources ignore it.
    pub fn force_hw(&self) {
        if let Source::Hw { shared, .. } = self {
            let mut state = crate::perfguard::deep::ui_lock(shared);
            state.force = true;
            state.record_flow.wake();
            #[cfg(test)]
            {
                state.force_requests += 1;
            }
        }
    }

    /// Hardware source in STOP: the view shown (time per div, delay; None: none to read for), and the frame the
    /// thread has read again from the stored record for it, if a new one came (`board::Shared::stop_view`).
    pub(crate) fn hw_stop(&mut self, view: Option<StopView>) -> (Option<ReplayLoss>, Option<Frame>) {
        if let Source::Hw { shared, .. } = self {
            let mut g = crate::perfguard::deep::ui_lock(shared);
            if g.stop_view != view {
                g.stop_view = view;
                g.record_flow.wake();
            }
            let reply = g.stop_replies.frame.take();
            if let Some(view) = view {
                let loss = g.stop_replies.loss_for(view.capture);
                let frame = reply.filter(|(key, _)| *key == view).map(|(_, frame)| frame);
                return (loss, frame);
            }
        }
        (None, None)
    }

    pub(crate) fn hw_stop_loss(&self, sequence: u64) -> Option<ReplayLoss> {
        if let Source::Hw { shared, .. } = self {
            return crate::perfguard::deep::ui_lock(shared).stop_replies.loss_for(sequence);
        }
        None
    }

    /// Drain a decoded batch fallback up to its newest record, which still needs full analysis and history.
    pub fn drain_newest(&mut self) -> Vec<std::sync::Arc<crate::board::Frame>> {
        if let Source::Hw { shared, .. } = self {
            let mut g = crate::perfguard::deep::ui_lock(shared);
            let id = g.acquisition.identity();
            let before = g.queue.len();
            g.queue.retain(|f| (f.cfg, f.stamp) == (id.gen(), id.epoch()));
            crate::drawpipe::Counters::add(
                &crate::drawpipe::counters().losses.queue_pop,
                (before - g.queue.len()) as u64,
            );
            // Leave interleaved fallbacks for ordered reader intake.
            if g.queue.len() > 1 && drawpipe::pipe().is_some_and(|p| p.has_waveform(id)) {
                return Vec::new();
            }
            let prefix = g.queue.len().saturating_sub(1);
            if prefix == 0 && before != 0 && g.queue.is_empty() {
                g.record_flow.drained();
            }
            return g.queue.drain(..prefix).collect();
        }
        Vec::new()
    }

    pub(crate) fn note_record_draw(&self, records: u64, draw: std::time::Duration) {
        if let Source::Hw { shared, .. } = self {
            if records > 0 {
                let mut g = crate::perfguard::deep::ui_lock(shared);
                let id = g.acquisition.identity();
                g.record_flow.cost.observe(id, records, draw, std::time::Duration::ZERO);
            }
        }
    }

    pub(crate) fn acquire_hw(
        &mut self,
        m: &Model,
        take_tap: impl FnOnce(oos_core::change::Identity, Option<u64>) -> TapRead<QueuedFrame>,
    ) -> Option<Acquired> {
        let Source::Hw { shared, seen } = self else { return None };
        let mut g = crate::perfguard::deep::ui_lock(shared);
        let id = g.acquisition.identity();
        let had_frames = !g.queue.is_empty();
        while g.queue.pop_front_if(|f| (f.cfg, f.stamp) != (id.gen(), id.epoch())).is_some() {
            drawpipe::Counters::add(&drawpipe::counters().losses.queue_pop, 1);
        }
        if had_frames && g.queue.is_empty() {
            g.record_flow.drained();
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
                Some(frame) => {
                    if g.queue.is_empty() {
                        g.record_flow.drained();
                    }
                    (QueuedFrame::new(frame, PictureEligibility::Drawable), false)
                }
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
        let (r, average) = f.read(m);
        if deep {
            crate::deepmeas::cost::set(&crate::deepmeas::cost::RECORD_US, t.elapsed());
        }
        Some((r, eligibility, tap, average))
    }
}

pub(super) fn hold(shared: &crate::board::HwShared, hold: bool, req: u64) -> bool {
    let mut g = crate::perfguard::deep::ui_lock(shared);
    if Source::stage_hold(&mut g, hold, req) {
        g.record_flow.wake();
    }
    g.final_ack >= req && g.queue.is_empty()
}
