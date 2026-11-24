// SPDX-License-Identifier: GPL-2.0-or-later

use std::sync::Arc;

#[derive(Default)]
pub(crate) struct RecordFlow {
    pub(crate) cost: crate::drawpipe::admission::Cost,
    pub(crate) source: Arc<crate::drawpipe::SourceWake>,
    pub(crate) shutdown: bool,
    /// The source may have a serial capture in flight that a plain STOP needs to drain.
    pub(crate) serial: bool,
    segments: Option<SoftwareSegments>,
}

struct SoftwareSegments {
    remaining: u32,
    drawing: bool,
}

impl RecordFlow {
    /// The UI starts a quota for each software-segmented record-route run.
    pub(crate) fn start_segments(&mut self, remaining: Option<u32>) {
        self.segments = remaining.map(|remaining| SoftwareSegments { remaining, drawing: false });
    }

    pub(crate) fn segment_bound(&self) -> Option<u32> {
        self.segments.as_ref().map(|state| if state.drawing { 0 } else { state.remaining })
    }

    pub(crate) fn segments_published(&mut self) {
        if let Some(state) = &mut self.segments {
            state.drawing = true;
        }
    }

    pub(crate) fn segments_pending(&self) -> bool {
        self.segments.as_ref().is_some_and(|state| state.drawing)
    }

    /// Reopen the quota once the UI has drawn or filtered the entire hand-off.
    pub(crate) fn segments_drawn(&mut self, remaining: u32) {
        if self.segments_pending() {
            self.start_segments(Some(remaining));
            self.drained();
        }
    }

    pub(crate) fn wake(&self) {
        self.source.notify();
        crate::drawpipe::wake_source();
    }

    pub(crate) fn drained(&self) {
        self.source.notify();
        if let Some(source) = crate::drawpipe::source_event() {
            source.notify();
        }
    }
}
