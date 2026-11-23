// SPDX-License-Identifier: GPL-2.0-or-later

//! Opt-in batch attribution. The coordinator owns the view; wide searches are sampled once around placement.

use super::{Route, RouteCounters};
use crate::drawpipe::View;
use std::fmt::Write;
use std::{
    ops::Range,
    sync::{atomic::Ordering, Arc, OnceLock},
};

pub struct RouteTrace {
    totals: RouteCounters,
    placement: OnceLock<Placement>,
    range: Range<usize>,
    #[cfg(test)]
    replay_prior_hits: Option<u64>,
    sink: Option<Box<dyn FnOnce(RouteReport) + Send + Sync>>,
}

struct Placement {
    view: Arc<View>,
    workers: usize,
    prior_hits: u64,
}

pub(crate) struct RouteReport {
    placement: Option<Placement>,
    range: Range<usize>,
    route: String,
}

impl RouteTrace {
    pub(crate) fn new(sink: impl FnOnce(RouteReport) + Send + Sync + 'static) -> Box<Self> {
        Box::new(Self {
            totals: RouteCounters::default(),
            placement: OnceLock::new(),
            range: 0..0,
            sink: Some(Box::new(sink)),
            #[cfg(test)]
            replay_prior_hits: None,
        })
    }

    pub(in crate::drawpipe) fn submitted(&mut self, range: Range<usize>) {
        self.range = range;
    }

    pub(in crate::drawpipe) fn claimed(&self, view: &Arc<View>, workers: usize, prior_hits: u64) {
        let _ = self.placement.set(Placement { view: view.clone(), workers, prior_hits });
    }

    #[cfg(test)]
    pub(crate) fn set_replay_prior_hits(&mut self, prior_hits: u64) {
        self.replay_prior_hits = Some(prior_hits);
    }

    #[cfg(test)]
    pub(in crate::drawpipe) fn replay_prior_hits(&self) -> Option<u64> {
        self.replay_prior_hits
    }

    pub(in crate::drawpipe) fn add(&self, route: Route) {
        self.count(route, 1);
    }

    pub(in crate::drawpipe) fn count(&self, route: Route, records: u64) {
        self.totals.add(route, records);
    }

    pub(in crate::drawpipe) fn add_wide_searches(&self, searches: u64) {
        self.totals.wide_search.fetch_add(searches, Ordering::Relaxed);
    }
}

impl Drop for RouteTrace {
    fn drop(&mut self) {
        if let Some(sink) = self.sink.take() {
            let mut route = String::new();
            self.totals.write(&mut route);
            sink(RouteReport { placement: self.placement.take(), range: self.range.clone(), route });
        }
    }
}

impl RouteReport {
    pub(crate) fn append_view(&self, out: &mut String) {
        let Some(Placement { view, workers, prior_hits }) = &self.placement else { return };
        let (w, h, interp) = (view.w, view.h, view.interp);
        let _ = writeln!(out, "view={w} {h} {interp:?} {workers} {prior_hits}");
        if let Some(zoom) = view.zoom {
            let ((w, h), factor, center) = zoom.fixture_geometry();
            let _ = writeln!(out, "zoom_view={w} {h} {factor} {center}");
        }
        for line in
            crate::store::setup::setup_text_matching(&view.m, fixture_view_key).lines().filter(|l| !l.starts_with('#'))
        {
            let _ = writeln!(out, "view.{line}");
        }
    }

    pub(crate) fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub(crate) fn route(&self) -> &str {
        self.route.trim_start()
    }
}

// The setup codec writes only typed placement and trigger inputs. User strings (labels,
// custom math, decoder names, paths, etc.) never enter the sidecar.
const VIEW_KEYS: &str = concat!(
    "holdoff trig_mode la dec_la dec_thr dec_opt dec_diff dec_src2 dec_cs dec_mode dec_bits dec_parity ",
    "dec_arate dec_tick dec_nibbles dec_uspeed dec_cphy_rate ",
    "tdiv tfine delay dots acq avg_count avg_hw zoom zoom_tdiv zdelay trig_src trig_ref trig_la pat_la trig_level slope ",
    "trig_type trig_level2 trig_nrej auto_nrej trig_coup trig_src2 trig_ref2 trig_slope2 trig_pol trig_when ",
    "trig_t1 trig_t2 video_std video_sync video_line pat pat_levels win_mode nth_idle nth_n setup hold meas ",
    "meas2 cat meas_full_record trig_bus trig_bus_val dec_sel dec_src dec_baud dec_proto dec_canrate dec_canh ",
    "dec_fdrate dec_src4 ser.rel ser.max ser.mask ser.uart_inv ser.uart_stop ser.uart_err ser.spi_cs_high ",
    "ser.spi_timeout ser.addr_rel ser.addr_max ser.dir ser.ten_bit ser.bytes ",
);

pub(crate) fn fixture_view_key(key: &str) -> bool {
    VIEW_KEYS.split_ascii_whitespace().any(|k| k == key)
        || (1..=4).any(|c| key.strip_prefix("deskew_ch").is_some_and(|n| n.as_bytes() == [b'0' + c]))
        || key.strip_prefix("ch").and_then(|key| key.split_once('.')).is_some_and(|(c, field)| {
            matches!(c, "1" | "2" | "3" | "4")
                && matches!(field, "on" | "vdiv" | "vfine" | "offset" | "probe" | "invert" | "coupling" | "bw")
        })
}
