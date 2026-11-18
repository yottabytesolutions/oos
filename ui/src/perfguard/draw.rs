// SPDX-License-Identifier: GPL-2.0-or-later

use super::{nanos, ring::Ring};
use std::time::Duration;

const LABELS: [&str; 7] =
    ["draw.decay", "draw.fast", "draw.merge", "draw.records", "draw.grade", "draw.render", "draw.view_render"];

/// Five paired step rings and independent panel/view render rings, all inline and bounded.
#[derive(Clone, Default)]
pub(super) struct Timing {
    steps: [Ring; 7],
    merge: Ring,
    frames: Ring,
}
impl Timing {
    pub(super) fn step(&mut self, parts: [Duration; 5]) {
        for (ring, part) in self.steps.iter_mut().zip(parts) {
            ring.add(nanos(part));
        }
    }
    pub(super) fn render(&mut self, elapsed: Duration, view: bool) {
        self.steps[if view { 6 } else { 5 }].add(nanos(elapsed));
    }
    pub(super) fn merge(&mut self, elapsed: Duration) {
        self.merge.add(nanos(elapsed));
    }
    pub(super) fn picture(&mut self, frames: usize) {
        self.frames.add(frames as u64);
    }
    pub(super) fn text(&self) -> String {
        let mut text: String =
            self.steps.iter().zip(LABELS).map(|(ring, label)| ring.clone().text(label, "ns")).collect();
        text.push_str(&self.merge.clone().text_with_p90("draw.worker_merge", "ns"));
        text.push_str(&self.frames.clone().text_with_p90("draw.frames_per_picture", "frames"));
        text.push_str(&format!(" worker_merge_us={}", crate::framecost::merge_us()));
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_and_picture_rings_report_independent_quantiles_including_empty_steps() {
        let mut timing = Timing::default();
        assert!(timing.text().contains("draw.worker_merge n=0 total=0 p50_ns=pending p90_ns=pending p99_ns=pending"));
        for i in 1..=100 {
            timing.merge(Duration::from_nanos(i));
            timing.picture(0);
            timing.picture(i as usize);
        }
        let text = timing.text();
        assert!(text.contains("draw.worker_merge n=100 total=100 p50_ns=50 p90_ns=90 p99_ns=99 max_ns=100"));
        assert!(text.contains(
            "draw.frames_per_picture n=200 total=200 p50_frames=0 p90_frames=80 p99_frames=98 max_frames=100"
        ));
    }

    #[test]
    fn draw_rings_keep_zeros_pending_quantiles_wrap_and_separate_render() {
        let mut timing = Timing::default();
        assert!(timing.text().contains("draw.decay n=0 total=0 p50_ns=pending p99_ns=pending"));
        for i in 1..=400 {
            timing.step([Duration::from_nanos(i), Duration::ZERO, Duration::ZERO, Duration::ZERO, Duration::ZERO]);
        }
        let text = timing.text();
        assert!(text.contains("draw.decay n=256 total=400 p50_ns=272 p99_ns=398 max_ns=400"));
        assert!(text.contains("draw.fast n=256 total=400 p50_ns=0 p99_ns=0 max_ns=0"));
        assert!(text.contains("draw.render n=0 total=0 p50_ns=pending"));
        timing.render(Duration::from_nanos(9), false);
        assert!(timing.text().contains("draw.render n=1 total=1 p50_ns=9 p99_ns=pending max_ns=9"));
        timing.render(Duration::from_nanos(17), true);
        assert!(timing.text().contains("draw.view_render n=1 total=1 p50_ns=17 p99_ns=pending max_ns=17"));
        assert!(timing.text().contains("draw.render n=1 total=1 p50_ns=9 p99_ns=pending max_ns=9"));
        assert!(std::mem::size_of::<Timing>() < 19 * 1024);
    }
}
