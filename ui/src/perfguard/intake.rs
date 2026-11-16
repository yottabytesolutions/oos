// SPDX-License-Identifier: GPL-2.0-or-later

//! Exclusive intake components paired with the existing completed-step ring.
use super::{deep, nanos};
use std::time::Instant;

#[derive(Clone)]
struct External {
    start: Instant,
    wait: u64,
    polls: u64,
    poll_wait: u64,
}

#[derive(Clone)]
pub(super) struct Timing {
    samples: [[u64; 4]; 256],
    pending: [u64; 3],
    queued: u64,
    poll_wait: u64,
    external: Option<External>,
    depth: usize,
}
impl Default for Timing {
    fn default() -> Self {
        Self { samples: [[0; 4]; 256], pending: [0; 3], queued: 0, poll_wait: 0, external: None, depth: 0 }
    }
}
impl Timing {
    pub(super) fn active(&self) -> bool {
        self.depth > 0
    }
    pub(super) fn begin(&mut self) {
        if !self.active() {
            self.external = Some(External {
                start: Instant::now(),
                wait: deep::counters().ui_wait(),
                polls: self.pending[0],
                poll_wait: self.poll_wait,
            });
        }
        self.depth += 1;
    }
    pub(super) fn end(&mut self) {
        self.depth = self.depth.saturating_sub(1);
        if self.active() {
            return;
        }
        if let Some(outer) = self.external.take() {
            let elapsed = outer.start.elapsed();
            let wait = deep::counters().ui_wait().wrapping_sub(outer.wait);
            let polls = self.pending[0].saturating_sub(outer.polls);
            let poll_wait = self.poll_wait.saturating_sub(outer.poll_wait);
            self.pending[1] = self.pending[1].saturating_add(nanos(elapsed).saturating_sub(polls).saturating_sub(wait));
            self.pending[2] = self.pending[2].saturating_add(wait.saturating_sub(poll_wait));
            self.queue(nanos(elapsed));
            deep::counters().poll(deep::Poll::External, elapsed);
        }
    }
    pub(super) fn queue(&mut self, elapsed: u64) {
        self.queued = self.queued.saturating_add(elapsed);
    }
    pub(super) fn poll(&mut self, elapsed: u64, wait: u64) {
        self.pending[0] = self.pending[0].saturating_add(elapsed.saturating_sub(wait));
        self.pending[2] = self.pending[2].saturating_add(wait);
        self.poll_wait = self.poll_wait.saturating_add(wait);
    }
    pub(super) fn finish(&mut self, index: usize, intake: u64) -> u64 {
        let total = intake.saturating_add(std::mem::take(&mut self.queued));
        let mut remaining = total;
        // Bound at the represented elapsed time, including at u64 saturation.
        let parts = self.pending.map(|part| {
            let part = part.min(remaining);
            remaining -= part;
            part
        });
        self.samples[index] = [parts[0], parts[1], parts[2], remaining];
        self.pending = [0; 3];
        self.poll_wait = 0;
        total
    }
    pub(super) fn text(&self, intake: &[u64; 256], n: usize) -> String {
        let mut indices = std::array::from_fn::<_, 256, _>(|i| i);
        // The index breaks ties deterministically; components come from that same sample.
        indices[..n].sort_unstable_by_key(|&i| (intake[i], i));
        let mut text = String::new();
        for percent in [50usize, 99] {
            if n == 0 || (percent == 99 && n < 50) {
                text.push_str(&format!(" diag intake.p{percent} n={n} pending"));
                continue;
            }
            let index = indices[(n * percent).div_ceil(100) - 1];
            let [polls, external, wait, residual] = self.samples[index];
            text.push_str(&format!(
                " diag intake.p{percent} n={n} intake_ns={} polls_ns={polls} external_ns={external} lock_wait_ns={wait} residual_ns={residual}",
                intake[index]
            ));
        }
        text
    }
}

#[cfg(test)]
mod tests;
