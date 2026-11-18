// SPDX-License-Identifier: GPL-2.0-or-later

use super::{nanos, ring::Ring};
use std::time::Duration;

const PANEL_RENDER: usize = 5;
const VIEW_RENDER: usize = 6;
const GPU_UPLOAD: usize = 7;

const LABELS: [&str; 8] = [
    "draw.decay",
    "draw.fast",
    "draw.merge",
    "draw.records",
    "draw.grade",
    "draw.render",
    "draw.view_render",
    "gpu.upload",
];

/// Bounded frame diagnostics: paired draw steps, independent panel/view renders and completed panel uploads.
#[derive(Clone, Default)]
pub(super) struct FrameTiming([Ring; LABELS.len()]);
impl FrameTiming {
    pub(super) fn step(&mut self, parts: [Duration; 5]) {
        for (ring, part) in self.0.iter_mut().zip(parts) {
            ring.add(nanos(part));
        }
    }
    pub(super) fn render(&mut self, elapsed: Duration, view: bool) {
        self.0[if view { VIEW_RENDER } else { PANEL_RENDER }].add(nanos(elapsed));
    }
    pub(super) fn gpu_upload(&mut self, elapsed: Duration) {
        self.0[GPU_UPLOAD].add(nanos(elapsed));
    }
    pub(super) fn text(&self) -> String {
        self.0.iter().zip(LABELS).map(|(ring, label)| ring.clone().text(label, "ns")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_rings_keep_zeros_pending_quantiles_wrap_and_separate_render() {
        let mut timing = FrameTiming::default();
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
        assert!(std::mem::size_of::<FrameTiming>() < 17 * 1024);
    }
}
