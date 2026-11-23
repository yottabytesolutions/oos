// SPDX-License-Identifier: GPL-2.0-or-later

use crate::board::Frame;
use crate::model::{AcqMode, Model, TrigType, VSync, HDIV};
use crate::picture::deposit::{kernel::Projection, standard};
use std::fmt::Write;
use std::sync::{Arc, Weak};

use super::{Batch, CodeLayout, View};
use crate::picture::PerMap;
#[cfg(any(target_os = "linux", test))]
mod trace;
mod zoom;
use crate::picture::deposit::envelope::Layout;
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(test)]
pub(crate) use trace::fixture_view_key;
#[cfg(any(target_os = "linux", test))]
pub(crate) use trace::RouteReport;
#[cfg(any(target_os = "linux", test))]
pub(crate) use trace::RouteTrace;
pub(crate) use zoom::ZoomView;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Route {
    Dense,
    Peak,
    Volts,
    Pairs,
    Dots,
    Upsampled,
    Record,
    Boxcar,
    Screen,
    Zoom,
}

/// Pick the placement adapter from the record's own form and the frozen view.
pub(crate) fn route(frame: &Frame, proj: &Projection, mode: AcqMode, codes: bool) -> Route {
    if frame.roll || frame.scan {
        Route::Record
    } else if frame.envelope && mode != AcqMode::HiRes {
        Route::Pairs
    } else if crate::deepmeas::deep_raw_frame(frame) && mode != AcqMode::HiRes {
        Route::Screen
    } else if mode == AcqMode::HiRes {
        let dt = proj.time_per_div * HDIV as f64 / crate::sparse::POINTS as f64;
        if dt / frame.dt >= 2.0 {
            Route::Boxcar
        } else {
            Route::Upsampled
        }
    } else if proj.x_per_sample > 0.5 {
        Route::Upsampled
    } else if proj.dots {
        Route::Dots
    } else if mode == AcqMode::Peak {
        Route::Peak
    } else if codes {
        Route::Dense
    } else {
        Route::Volts
    }
}

impl Route {
    pub(super) fn for_dma(self, projection: &Projection, layout: Option<&CodeLayout>) -> Self {
        if self == Self::Upsampled && projection.dots && layout.is_some_and(CodeLayout::reduced_capture) {
            Self::Dots
        } else {
            self
        }
    }

    /// Preserve weighted segments on the legacy screen grid for deep pairs and exact interpolation.
    pub(crate) fn screen_raster(self, dots: bool, frame: &Frame) -> bool {
        matches!(self, Self::Record | Self::Upsampled | Self::Screen)
            || self == Self::Pairs && crate::deepmeas::deep_pairs(frame)
            || self == Self::Boxcar && dots
    }
}

/// The TPU implements the native trigger conditions. Bus can arm on a start edge
/// while software looks for the actual match. Video All Fields is also a software selection.
pub(crate) fn hardware_trigger(m: &Model) -> bool {
    match m.trig_type {
        TrigType::Edge | TrigType::Bus => false,
        TrigType::Video => m.video_sync != VSync::AllFields,
        TrigType::Pulse
        | TrigType::Slope
        | TrigType::Pattern
        | TrigType::Duration
        | TrigType::Timeout
        | TrigType::Runt
        | TrigType::Window
        | TrigType::Delay
        | TrigType::SetupHold
        | TrigType::NthEdge => true,
    }
}

pub(crate) fn hardware_position(m: &Model, frame: &Frame) -> Option<f64> {
    frame.fixed_ti.or_else(|| hardware_trigger(m).then_some(frame.pre as f64))
}

struct ViewProjection {
    view: Weak<View>,
    projection: PerMap<Projection>,
    code_geometry: bool,
}

#[derive(Default)]
pub(crate) struct Scratch {
    projection: Option<ViewProjection>,
    layout: Option<BatchLayout>,
    pub(crate) zoom: standard::ZoomScratch,
}

pub(super) fn projection(view: &Arc<View>, dt: f64) -> (PerMap<Projection>, bool) {
    super::stage::fixture_stage!(Geometry, cached_projection(view, dt))
}

fn cached_projection(view: &Arc<View>, dt: f64) -> (PerMap<Projection>, bool) {
    let spacing = dt / view.m.time_per_div() as f64 * view.w as f64 / crate::model::HDIV as f64;
    crate::fastdraw::with_scratch(|scratch| {
        let cached = &mut scratch.routing.projection;
        if let Some(saved) = cached {
            if saved.view.as_ptr() == Arc::as_ptr(view) && saved.projection.main().x_per_sample == spacing {
                return (saved.projection, saved.code_geometry);
            }
        }
        let projection = standard::projection(&view.m, view.w, view.h, spacing);
        let code_geometry = projection.code_geometry();
        let projection = PerMap::new(projection, view.zoom.map(|zoom| zoom.projection(&view.m, dt)));
        *cached = Some(ViewProjection { view: Arc::downgrade(view), projection, code_geometry });
        (projection, code_geometry)
    })
}

struct BatchLayout {
    batch: Weak<Batch>,
    layout: Option<Layout>,
}

pub(super) fn layout(batch: &Arc<Batch>, raw: &CodeLayout) -> Option<Layout> {
    super::stage::fixture_stage!(Geometry, cached_layout(batch, raw))
}

fn cached_layout(batch: &Arc<Batch>, raw: &CodeLayout) -> Option<Layout> {
    crate::fastdraw::with_scratch(|scratch| {
        let cached = &mut scratch.routing.layout;
        if let Some(saved) = cached {
            if saved.batch.as_ptr() == Arc::as_ptr(batch) {
                return saved.layout;
            }
        }
        let layout = Layout::new(raw);
        *cached = Some(BatchLayout { batch: Arc::downgrade(batch), layout });
        layout
    })
}

const LABELS: [&str; 10] =
    ["dense", "peak", "volts", "pairs", "dots", "upsampled", "record", "boxcar", "screen", "zoom"];

#[derive(Default)]
pub(crate) struct RouteCounters {
    records: [AtomicU64; LABELS.len()],
    wide_search: AtomicU64,
    pub(crate) fallbacks: super::metrics::fallback::Counters,
}

impl RouteCounters {
    pub(crate) fn add(&self, route: Route, records: u64) {
        self.records[route as usize].fetch_add(records, Ordering::Relaxed);
    }

    pub(crate) fn wide_search(&self) {
        self.wide_search.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn wide_searches(&self) -> u64 {
        self.wide_search.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(crate) fn count(&self, route: Route) -> u64 {
        self.records[route as usize].load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(super) fn total(&self) -> u64 {
        self.records.iter().map(|count| count.load(Ordering::Relaxed)).sum()
    }

    pub(super) fn write(&self, text: &mut String) {
        text.push_str(" route.v1");
        for (label, count) in LABELS.into_iter().zip(&self.records) {
            let _ = write!(text, " {label}={}", count.load(Ordering::Relaxed));
        }
        let _ = write!(text, " wide_search={}", self.wide_searches());
    }
}

#[cfg(test)]
mod tests;
