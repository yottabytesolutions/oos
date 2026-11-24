// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{AView, AcqMode, DotsMode, Model, RunState, WinLayout};
use std::time::{Duration, Instant};

// About one second of pictures, enough to distinguish repeated misses from individual slow frames.
const WINDOW: usize = 32;
// One slow picture in the window is an outlier. Two are a pattern: about 6 % of the pictures would miss.
const STAY_RANK: usize = WINDOW - 1;
const QUEUE_MARGIN: Duration = Duration::from_micros(500);
const HYSTERESIS: Duration = Duration::from_millis(1);
const HOLD: Duration = Duration::from_secs(3);

/// Changes that alter the work of composing and rendering the view.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct CostView {
    page: (AView, usize),
    channels: u8,
    main_scale: usize,
    zoom_scale: Option<usize>,
    depth: usize,
    measurements: (usize, usize),
    mode: (AcqMode, RunState),
    style: (WinLayout, DotsMode, bool),
    gpu: bool,
}

impl CostView {
    pub(crate) fn new(model: &Model, gpu: bool) -> Self {
        Self {
            page: (model.view, model.view_cur),
            channels: model.ch.iter().enumerate().fold(0, |mask, (n, ch)| mask | (u8::from(ch.on) << n)),
            main_scale: model.tscale,
            zoom_scale: model.zoom.then_some(model.zscale),
            depth: model.mdepth,
            measurements: (model.meas.len(), model.meas2.len()),
            mode: (model.acq, model.run),
            style: (model.win_layout, model.dots, model.color_grade),
            gpu,
        }
    }
}

pub(super) struct Target {
    view: Option<CostView>,
    samples: [Duration; WINDOW],
    next: usize,
    count: usize,
    changed: Option<Instant>,
    refreshes: u32,
}

impl Default for Target {
    fn default() -> Self {
        Self { view: None, samples: [Duration::ZERO; WINDOW], next: 0, count: 0, changed: None, refreshes: 2 }
    }
}

impl Target {
    pub(super) fn select_view(&mut self, view: CostView) {
        if self.view != Some(view) {
            self.view = Some(view);
            self.next = 0;
            self.count = 0;
        }
    }

    pub(super) fn observe(&mut self, at: Instant, cost: Duration, refresh: Duration) {
        self.samples[self.next] = cost;
        self.next = (self.next + 1) % WINDOW;
        self.count = (self.count + 1).min(WINDOW);
        if self.count < WINDOW || self.changed.is_some_and(|last| at.saturating_duration_since(last) < HOLD) {
            return;
        }
        let budget = (refresh * 2).saturating_sub(QUEUE_MARGIN);
        let returning = self.refreshes == 3;
        let threshold = if returning { budget.saturating_sub(HYSTERESIS) } else { budget };
        // Leaving two refreshes takes two misses in the window. Returning takes a window without any, below the
        // threshold: a view on the boundary then keeps an even three instead of alternating. A scan avoids sorting.
        let rank = if returning { WINDOW } else { STAY_RANK };
        let fits = self.samples.iter().filter(|&&sample| sample <= threshold).count() >= rank;
        if fits == returning {
            self.refreshes = if returning { 2 } else { 3 };
            self.changed = Some(at);
        }
    }

    pub(super) fn refreshes(&self) -> u32 {
        self.refreshes
    }
}

#[cfg(test)]
mod tests;
