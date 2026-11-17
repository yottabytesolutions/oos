// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The second pane under the plot: the frame of the spectrum (also the full view) with its peak list, the results table
//! and the trend charts. Everything that takes time happens per record or per trend point, the drawing only reads it.
//! A pane is as high as its content; what is left over goes to the plot.

mod spectrum_view;

use super::*;

pub(super) const TREND_RESETS_ON: oos_core::change::Classes =
    oos_core::change::Classes::STATS.union(oos_core::change::Classes::ACQ);
use oos_dsp::spectrum;
use std::collections::VecDeque;

/// Harmonics of the measured frequency the span of the pane covers; 0 = the whole band.
pub const PANE_HARMS: [usize; 5] = [0, 5, 10, 20, 50];
/// Values of `Model::trend_sel`: by the list, off, one per measurement.
pub const TREND_SEL_N: usize = tools::MEAS_MAX + 2;
/// Lines in the peak list and on the trace.
pub(super) const PEAKS_MAX: usize = super::analysis::peaks::MAX;
pub(super) const DB_PER_DIV: f32 = 10.0;

/// Peak list width; left axis; title line and frequency labels of the spectrum.
const PEAK_W: i32 = 212;
const AXIS_W: i32 = 34;
const TITLE_H: i32 = 20;
const XLABEL_H: i32 = 16;
/// Height of a pane with only a line of text.
const HINT_H: i32 = 60;
/// Band above the results table, as high as the close button.
const BAND_H: i32 = 40;
/// Row heights of the results table.
const ROW_MIN: i32 = 19;
const ROW_WANT: i32 = 26;
/// Width of the side column of the Results pane.
const SIDE_W: i32 = 300;
/// Time a shorter list of events waits before the pane shrinks (s): the plot should not jump with every record.
const SHRINK_AFTER: f64 = 3.0;

pub(super) fn harm_names() -> Vec<String> {
    PANE_HARMS.iter().map(|&h| if h == 0 { "All".to_string() } else { h.to_string() }).collect()
}

/// Columns and rows per column for `n` events in a height: one column while they are few.
pub(super) fn event_grid(n: usize, h: i32) -> (usize, usize) {
    let fit = (h / 20).max(1) as usize;
    let cols = if n <= 4 { 1 } else { 2 };
    (cols, n.div_ceil(cols).clamp(1, fit))
}

/// What the second pane keeps between frames.
#[derive(Default)]
pub struct PaneState {
    /// first row of the results table
    pub top: usize,
    /// decoder events, refreshed a few times a second
    pub events: Vec<tables::EventRow>,
    events_t: f64,
    /// rows the Events pane is high enough for, and when the list last needed them
    event_rows: usize,
    event_seen: f64,
    /// the Bus view: the views of the protocol modules, refreshed like the events
    pub views: Vec<(String, crate::protocols::View)>,
    /// per decoder: what its module keeps between records, and the view of the record last looked at
    pub view_slots: [super::busview::ViewSlot; DECODERS],
}

/// Where the spectrum's parts go.
pub(super) struct SpecBox {
    pub title: Rect,
    pub plot: Rect,
    /// the sort button, and the list under it
    pub sort: Rect,
    pub list: Rect,
}

/// The nearest "round" step at or above `x`.
fn round_step(x: f64) -> f64 {
    if !(x > 0.0) || !x.is_finite() {
        return 1.0;
    }
    let d = 10f64.powf(x.log10().floor());
    [1.0, 2.0, 2.5, 5.0, 10.0].iter().map(|k| k * d).find(|&s| s >= x * (1.0 - 1e-9)).unwrap_or(10.0 * d)
}

/// Grid of a chart: `nx` by `ny` divisions.
fn chart_grid(cv: &mut Canvas, g: Rect, nx: i32, ny: i32) {
    for i in 1..nx {
        cv.vline(g.x + i * (g.w - 1) / nx, g.y, g.y + g.h - 1, GRID_LINE);
    }
    for j in 1..ny {
        cv.hline(g.x, g.x + g.w - 1, g.y + j * (g.h - 1) / ny, GRID_LINE);
    }
    cv.rect(g, GRID_AXIS);
}

/// The largest of `fonts` in which every text is at most `w` wide.
fn fit_font<'a>(fonts: &[&'a Font], w: i32, texts: impl Iterator<Item = &'a str> + Clone) -> &'a Font {
    let fits = |f: &Font| texts.clone().all(|t| f.width(t) <= w);
    fonts.iter().copied().find(|f| fits(f)).unwrap_or(fonts[fonts.len() - 1])
}

// ---------------------------------------------------------------------------------------------------------------
// spectrum

// ---------------------------------------------------------------------------------------------------------------
// results

/// Columns of the results table: heading, relative width. The last one is a bar and goes first when room is short.
const RESULT_COLS: [(&str, i32); 9] = [
    ("Source", 54),
    ("Measurement", 136),
    ("Now", 88),
    ("Mean", 88),
    ("Min", 88),
    ("Max", 88),
    ("\u{3c3}", 80),
    ("Count", 60),
    ("Range", 90),
];

/// Left edges of the columns in a table `w` wide, and the end.
fn result_cols(w: i32) -> Vec<i32> {
    let sum: i32 = RESULT_COLS.iter().map(|c| c.1).sum();
    let bar = w >= sum;
    let used = sum - if bar { 0 } else { RESULT_COLS[8].1 };
    let mut x = 0;
    let mut v = vec![0];
    for c in &RESULT_COLS[..if bar { 9 } else { 8 }] {
        x += c.1 * w / used;
        v.push(x);
    }
    v
}

#[cfg(test)]
pub(super) fn tests_result_cols(w: i32) -> Vec<i32> {
    result_cols(w)
}

/// The cells of a results row.
struct ResultRow {
    ch: WaveId,
    cells: [String; 8],
    now: f64,
    lo: f64,
    hi: f64,
}

impl Ui {
    pub(super) fn spec_wf_h(&self, a: Rect, pane: bool) -> i32 {
        spectrum_view::spec_wf_h(self, a, pane)
    }

    pub(super) fn spec_box(&self, a: Rect, pane: bool) -> SpecBox {
        spectrum_view::spec_box(self, a, pane)
    }

    fn peaks_shown(&self, b: &SpecBox) -> usize {
        spectrum_view::peaks_shown(self, b)
    }

    pub(super) fn sort_toggle(&mut self) {
        spectrum_view::sort_toggle(self);
    }

    pub(super) fn draw_spec_frame(&self, cv: &mut Canvas, b: &SpecBox, sp: &spectrum::Spectrum) {
        spectrum_view::draw_spec_frame(self, cv, b, sp);
    }

    pub(super) fn draw_spec_marks(&self, cv: &mut Canvas, b: &SpecBox, sp: &spectrum::Spectrum, top: f32) {
        spectrum_view::draw_spec_marks(self, cv, b, sp, top);
    }

    pub(super) fn draw_peaks(&self, cv: &mut Canvas, b: &SpecBox, sp: &spectrum::Spectrum, pane: bool) {
        spectrum_view::draw_peaks(self, cv, b, sp, pane);
    }

    fn draw_results_side(&self, cv: &mut Canvas, s: Rect) {
        spectrum_view::draw_results_side(self, cv, s);
    }

    /// Rows of the side column of the Results pane: cursors, peak list.
    fn results_side_rows(&self) -> i32 {
        let cur = match self.m.cursor {
            CursorMode::Off => 0,
            CursorMode::Time | CursorMode::Volt => 3,
            _ => 4,
        };
        cur + if self.wave_ready(self.m.spec_src) { 5 } else { 0 }
    }

    /// Height the current pane wants.
    pub(super) fn pane_want(&self) -> i32 {
        let n = (self.m.meas.len() + self.m.meas2.len()) as i32;
        match self.m.pane {
            tables::PANE_RESULTS => {
                let side = self.results_side_rows();
                if n == 0 && side == 0 {
                    return HINT_H;
                }
                (BAND_H + n.max(1) * ROW_WANT).max(BAND_H + side * 18)
            }
            tables::PANE_EVENTS if self.pn.event_rows > 0 => self.pn.event_rows as i32 * 24,
            tables::PANE_EVENTS => HINT_H,
            tables::PANE_BUS if self.pn.views.is_empty() => HINT_H,
            tables::PANE_BUS => self.bus_view_h(),
            tables::PANE_TREND if n == 0 => HINT_H,
            tables::PANE_SPECTRUM if !self.wave_ready(self.m.spec_src) => HINT_H,
            _ => i32::MAX / 2,
        }
    }

    /// Per frame: the slow content of the pane, a few times a second.
    pub(super) fn pane_tick(&mut self) {
        if !matches!(self.m.pane, tables::PANE_EVENTS | tables::PANE_BUS) || (self.now - self.pn.events_t).abs() < 0.25
        {
            return;
        }
        self.pn.events_t = self.now;
        if self.m.pane == tables::PANE_BUS {
            self.pn.views = self.bus_views();
            return;
        }
        self.pn.events = self.decode_events();
        let n = self.pn.events.len();
        let want = n.div_ceil(if n <= 4 { 1 } else { 2 });
        if want >= self.pn.event_rows {
            (self.pn.event_rows, self.pn.event_seen) = (want, self.now);
        } else if self.now - self.pn.event_seen > SHRINK_AFTER {
            self.pn.event_rows = want;
        }
    }

    fn result_rows(&self) -> Vec<ResultRow> {
        self.meas_rows()
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let st = self.mstat.get(i).copied().unwrap_or_default();
                let n = st.n.max(1) as f64;
                let mean = st.sum / n;
                let sd = (st.sumsq / n - mean * mean).max(0.0).sqrt();
                let val = |v: f64| if st.n == 0 { "----".to_string() } else { r.fmt4(v) };
                // a pair names both channels
                let src = self
                    .m
                    .meas2
                    .get(i.wrapping_sub(self.m.meas.len()))
                    .map_or(self.wave_name(r.src), |p| format!("{}-{}", self.wave_name(p.1), self.wave_name(p.2)));
                ResultRow {
                    ch: r.src,
                    cells: [
                        src,
                        r.name.clone(),
                        r.fmt4(r.v),
                        val(mean),
                        val(st.min),
                        val(st.max),
                        val(sd),
                        fmt_count(st.n as f64),
                    ],
                    now: r.v,
                    lo: st.min,
                    hi: st.max,
                }
            })
            .collect()
    }

    /// Band, table rectangle, side column of the pane in `r`.
    fn results_box(&self, r: Rect) -> (Rect, Option<Rect>) {
        let side = self.results_side_rows() > 0;
        let w = if side { r.w - SIDE_W - 8 } else { r.w };
        (
            Rect::new(r.x, r.y, w, r.h),
            side.then(|| Rect::new(r.x + r.w - SIDE_W, r.y + BAND_H, SIDE_W - 6, r.h - BAND_H)),
        )
    }

    /// Rows that fit, and their pitch: the rows share the height of the pane.
    pub(super) fn results_fit(r: Rect, n: usize) -> (usize, i32) {
        let room = r.h - BAND_H;
        let fit = ((room / ROW_MIN).max(1) as usize).min(n.max(1));
        (fit, room / fit as i32)
    }

    /// The scroll bar of the Results pane and its rows, while the pane shows them.
    pub(super) fn results_bar(&self) -> Option<(scroll::Bar, Rect)> {
        if self.pane_h() == 0 || self.m.pane != tables::PANE_RESULTS {
            return None;
        }
        let r = self.pane_rect();
        let n = self.meas_rows().len();
        let (fit, pitch) = Self::results_fit(r, n);
        let (t, _) = self.results_box(r);
        let rows = Rect::new(t.x, t.y + BAND_H, t.w, fit as i32 * pitch);
        let track = Rect::new(t.x + t.w - scroll::BAR_W, rows.y, scroll::BAR_W, rows.h);
        Some((scroll::Bar { track, total: n as i32, shown: fit as i32, pos: self.pn.top as i32 }, rows))
    }

    /// Height of a row of the Results pane.
    pub(super) fn results_row_pitch(&self) -> i32 {
        Self::results_fit(self.pane_rect(), self.meas_rows().len()).1
    }

    fn draw_results_pane(&self, cv: &mut Canvas, r: Rect) {
        let f = self.fonts();
        let rows = self.result_rows();
        let (t, side) = self.results_box(r);
        if rows.is_empty() {
            cv.text(&f.sb, t.x + 12, t.y + (r.h + 9) / 2, "No measurements: tap Measure", DIM);
        } else {
            self.draw_result_table(cv, r, t, &rows);
        }
        if let Some(s) = side {
            cv.vline(s.x - 5, r.y + 4, r.y + r.h - 5, LINE);
            self.draw_results_side(cv, s);
        }
    }

    fn draw_result_table(&self, cv: &mut Canvas, r: Rect, t: Rect, rows: &[ResultRow]) {
        let f = self.fonts();
        let (fit, pitch) = Self::results_fit(r, rows.len());
        // a longer list keeps its scroll bar at the right of the rows
        let bar = (rows.len() > fit).then(|| scroll::Bar {
            track: Rect::new(t.x + t.w - scroll::BAR_W, t.y + BAND_H, scroll::BAR_W, fit as i32 * pitch),
            total: rows.len() as i32,
            shown: fit as i32,
            pos: self.pn.top as i32,
        });
        let t = if bar.is_some() { Rect::new(t.x, t.y, t.w - scroll::BAR_W - 4, t.h) } else { t };
        let top = self.pn.top.min(rows.len() - fit);
        let band = format!("Measurements  \u{b7}  {}", rows.len());
        let bx = cv.text(&f.sb, t.x + 12, t.y + 16, &band, TEXT);
        if rows.len() > fit {
            let s = format!("rows {} to {}", top + 1, top + fit);
            cv.text(&f.s, (t.x + 150).max(bx + 14), t.y + 16, &s, DIM);
        }
        let x = result_cols(t.w - 16);
        let x0 = t.x + 12;
        let head_y = t.y + BAND_H - 6;
        for (k, c) in RESULT_COLS.iter().enumerate().take(x.len() - 1) {
            // words on the left, numbers on the right of their column
            let tx = if k < 2 || k == 8 { x0 + x[k] } else { x0 + x[k + 1] - 8 - f.s.width(c.0) };
            cv.text(&f.s, tx, head_y, c.0, DIM);
        }
        cv.hline(t.x + 6, t.x + t.w - 6, t.y + BAND_H - 1, LINE);
        // the largest font every cell of a column fits in
        let col_w = (2..7).map(|k| x[k + 1] - x[k] - 6).min().unwrap_or(40);
        let shown = &rows[top..top + fit];
        // the normal size last: the larger text sizes fall back to it where a column is too narrow
        let n = crate::font::Fonts::get(0);
        let fonts: &[&Font] = if pitch >= 40 {
            &[&f.lb, &f.mb, &f.sb, &n.sb]
        } else if pitch >= 24 {
            &[&f.mb, &f.sb, &n.sb]
        } else {
            &[&f.sb, &n.sb]
        };
        let vf = fit_font(fonts, col_w, shown.iter().map(|w| w.cells[2].as_str()));
        let stats: &[&Font] = if pitch >= 24 { &[&f.m, &f.s, &n.s] } else { &[&f.s, &n.s] };
        let plain = fit_font(stats, col_w, shown.iter().flat_map(|w| w.cells[3..7].iter().map(|s| s.as_str())));
        for (i, w) in shown.iter().enumerate() {
            let y = t.y + BAND_H + i as i32 * pitch;
            let row = Rect::new(t.x, y, t.w, pitch);
            if i % 2 == 1 {
                cv.fill(row, ROW_ALT);
            }
            let col = self.wave_color(w.ch);
            cv.fill(Rect::new(t.x + 2, y + 1, 3, pitch - 2), col);
            let ty = y + (pitch + vf.size * 3 / 4) / 2;
            cv.text(&f.sb, x0 + x[0], ty, &w.cells[0], col);
            // the name in the figures' font, or the smallest of the text size it fits in
            let room = x[2] - x[1] - 6;
            let name_font = if plain.width(&w.cells[1]) <= room && plain.size >= f.s.size {
                plain
            } else {
                fit_font(&[&f.s, &n.s], room, std::iter::once(w.cells[1].as_str()))
            };
            cv.text(name_font, x0 + x[1], ty, &w.cells[1], LABEL);
            for k in 2..8 {
                let (font, c) = match k {
                    2 => (vf, col),
                    7 => (fit_font(&[&f.s, &n.s], x[8] - x[7] - 8, std::iter::once(w.cells[7].as_str())), DIM),
                    _ => (plain, LABEL),
                };
                cv.text(font, x0 + x[k + 1] - 8 - font.width(&w.cells[k]), ty, &w.cells[k], c);
            }
            if x.len() > 9 {
                Self::range_bar(cv, Rect::new(x0 + x[8], y + pitch / 2 - 3, x[9] - x[8] - 6, 6), w, col);
            }
        }
        if let Some(b) = bar {
            b.draw(cv);
        }
    }

    /// Where the value stands between the smallest and the largest seen.
    fn range_bar(cv: &mut Canvas, b: Rect, w: &ResultRow, col: Rgb) {
        cv.fill(Rect::new(b.x, b.y + b.h / 2, b.w, 1), LINE);
        cv.vline(b.x, b.y, b.y + b.h, LINE);
        cv.vline(b.x + b.w - 1, b.y, b.y + b.h, LINE);
        if w.hi > w.lo && w.now.is_finite() {
            let x = b.x + (((w.now - w.lo) / (w.hi - w.lo)).clamp(0.0, 1.0) * (b.w - 3) as f64) as i32;
            cv.fill(Rect::new(x, b.y - 1, 3, b.h + 2), col);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// trend

/// Points of a trend, and the time one covers at the start: when it is full, neighbours merge and the time doubles, so a
/// trend reaches back as far as the scope has run.
const TREND_PTS: usize = 480;
const TREND_STEP: f64 = 0.1;
/// Values of a chart's scale: the fraction cut off at each end (a stray record must not set the scale), the margin, the
/// smallest span relative to the value (the last digits are no signal), the hold before it tightens (points).
const SCALE_CUT: f64 = 0.02;
const SCALE_MARGIN: f64 = 0.15;
const SCALE_MIN_REL: f64 = 1e-3;
const SCALE_HOLD: u32 = 30;
/// Time axes of a chart, s: fifths of them are round numbers.
const TREND_WINDOWS: [f64; 16] =
    [10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 60.0, 75.0, 100.0, 150.0, 200.0, 300.0, 600.0, 1200.0, 3600.0];
/// Width of the value card and of the y labels of a chart.
const CARD_W: i32 = 128;
const YLABEL_W: i32 = 68;

/// A stretch of time of a trend: its mean, smallest and largest value.
#[derive(Clone, Copy, Debug)]
pub struct TrendPt {
    pub t: f64,
    pub v: f32,
    pub lo: f32,
    pub hi: f32,
}

/// History of one measurement.
#[derive(Clone, Debug)]
pub struct Trend {
    pub pts: VecDeque<TrendPt>,
    step: f64,
    /// open point: start, sum, count, smallest, largest
    open: (f64, f64, u32, f32, f32),
    /// the scale of the chart: bottom, top, tick distance
    pub scale: Option<(f64, f64, f64)>,
    /// points the data has been far inside the scale
    slack: u32,
    /// smallest and largest value of the history
    pub range: Option<(f64, f64)>,
}

impl Default for Trend {
    fn default() -> Trend {
        Trend {
            pts: VecDeque::new(),
            step: TREND_STEP,
            open: (f64::NAN, 0.0, 0, f32::MAX, f32::MIN),
            scale: None,
            slack: 0,
            range: None,
        }
    }
}

impl Trend {
    /// A new value at time `t`.
    pub fn push(&mut self, t: f64, v: f64) {
        if self.open.0.is_nan() {
            self.open.0 = t;
        }
        if t - self.open.0 >= self.step {
            self.close();
            self.open.0 = t;
        }
        if v.is_finite() {
            let o = &mut self.open;
            (o.1, o.2, o.3, o.4) = (o.1 + v, o.2 + 1, o.3.min(v as f32), o.4.max(v as f32));
        }
    }

    /// The open point goes into the history (a stretch without a value is a gap), the scale follows.
    fn close(&mut self) {
        let (t, sum, n, lo, hi) = self.open;
        let p = if n > 0 {
            TrendPt { t, v: (sum / n as f64) as f32, lo, hi }
        } else {
            TrendPt { t, v: f32::NAN, lo: f32::NAN, hi: f32::NAN }
        };
        self.pts.push_back(p);
        self.open = (t, 0.0, 0, f32::MAX, f32::MIN);
        if self.pts.len() >= TREND_PTS {
            self.merge();
        }
        self.rescale();
    }

    fn merge(&mut self) {
        let old = std::mem::take(&mut self.pts);
        let mut it = old.into_iter();
        while let Some(a) = it.next() {
            let p = match it.next() {
                Some(b) if a.v.is_nan() => b,
                Some(b) if b.v.is_nan() => a,
                Some(b) => TrendPt { t: a.t, v: (a.v + b.v) / 2.0, lo: a.lo.min(b.lo), hi: a.hi.max(b.hi) },
                None => a,
            };
            self.pts.push_back(p);
        }
        self.step *= 2.0;
    }

    /// Scale of the chart: the data without its extremes and a margin, on round ticks. It widens at once, and tightens
    /// only when the data has used less than half of it for a while, so noise does not make it jump.
    fn rescale(&mut self) {
        let mut v: Vec<f32> = self.pts.iter().map(|p| p.v).filter(|x| x.is_finite()).collect();
        if v.is_empty() {
            return;
        }
        self.range = self.pts.iter().filter(|p| p.lo.is_finite()).fold(None, |r: Option<(f64, f64)>, p| {
            Some(r.map_or((p.lo as f64, p.hi as f64), |(a, b)| (a.min(p.lo as f64), b.max(p.hi as f64))))
        });
        // where the data is now: the middle of the last three, a single stray point is not that
        let mut end: Vec<f32> = v[v.len().saturating_sub(3)..].to_vec();
        end.sort_by(f32::total_cmp);
        let last = end[end.len() / 2] as f64;
        v.sort_by(f32::total_cmp);
        let cut = (v.len() as f64 * SCALE_CUT) as usize;
        let (lo, hi) = ((v[cut] as f64).min(last), (v[v.len() - 1 - cut] as f64).max(last));
        let mid = (lo + hi) / 2.0;
        let span = (hi - lo).max(mid.abs() * SCALE_MIN_REL).max(1e-12);
        let (lo, hi) = (mid - span * (0.5 + SCALE_MARGIN), mid + span * (0.5 + SCALE_MARGIN));
        let snap = |lo: f64, hi: f64| {
            let tick = round_step((hi - lo) / 4.0);
            ((lo / tick).floor() * tick, (hi / tick).ceil() * tick, tick)
        };
        match self.scale {
            None => self.scale = Some(snap(lo, hi)),
            Some((a, b, _)) if lo < a || hi > b => {
                self.scale = Some(snap(lo.min(a), hi.max(b)));
                self.slack = 0;
            }
            Some((a, b, _)) if hi - lo < 0.5 * (b - a) => {
                self.slack += 1;
                if self.slack >= SCALE_HOLD {
                    self.scale = Some(snap(lo, hi));
                    self.slack = 0;
                }
            }
            _ => self.slack = 0,
        }
    }
}

impl Ui {
    /// Append new values. Accepted change classes own resets before record processing.
    pub(super) fn trend_update(&mut self, rows: &[tools::MeasRow]) {
        self.trend.resize(rows.len(), Trend::default());
        for (t, r) in self.trend.iter_mut().zip(rows) {
            t.push(self.now, r.v);
        }
    }

    /// Choices of a chart: by the list, off, the measurements.
    pub(super) fn trend_options(&self) -> Vec<String> {
        let mut v = vec!["Auto".to_string(), "Off".to_string()];
        // a pair says its channels in its name
        let single = self.m.meas.len();
        v.extend(self.meas_rows().iter().enumerate().map(|(i, r)| {
            if i < single {
                format!("{} {}", self.wave_name(r.src), r.name)
            } else {
                r.name.clone()
            }
        }));
        v
    }

    /// Rows the charts show, in chart order: (chart, row).
    pub(super) fn trend_charts(&self) -> Vec<(usize, usize)> {
        let n = self.meas_rows().len();
        (0..2)
            .filter_map(|k| match self.m.trend_sel[k] {
                1 => None,
                s if s >= 2 && s - 2 < n => Some((k, s - 2)),
                _ => (k < n).then_some((k, k)),
            })
            .collect()
    }

    /// The charts side by side.
    fn chart_rects(r: Rect, n: usize) -> Vec<Rect> {
        let w = (r.w - 8 * (n as i32 - 1).max(0)) / (n as i32).max(1);
        (0..n as i32).map(|i| Rect::new(r.x + i * (w + 8), r.y, w, r.h)).collect()
    }

    fn draw_trends(&self, cv: &mut Canvas, r: Rect) {
        let f = self.fonts();
        let rows = self.meas_rows();
        let charts = self.trend_charts();
        if rows.is_empty() || charts.is_empty() {
            let s = if rows.is_empty() {
                "No measurements: tap Measure"
            } else {
                "Both charts off: Display > Trend chart 1, 2"
            };
            cv.text(&f.sb, r.x + 12, r.y + (r.h + 9) / 2, s, DIM);
            return;
        }
        for (c, &(_, row)) in Self::chart_rects(r, charts.len()).into_iter().zip(&charts) {
            self.draw_chart(cv, c, &rows[row], self.trend.get(row));
        }
    }

    /// One chart: title and plot on the left, a card with the value on the right.
    fn draw_chart(&self, cv: &mut Canvas, c: Rect, row: &tools::MeasRow, t: Option<&Trend>) {
        let f = self.fonts();
        let col = self.wave_color(row.src);
        let x = cv.text(&f.sb, c.x + 8, c.y + 15, &self.wave_name(row.src), col);
        cv.text(&f.sb, x + 8, c.y + 15, &row.name, TEXT);
        let g = Rect::new(c.x + YLABEL_W, c.y + TITLE_H, c.w - YLABEL_W - CARD_W - 8, c.h - TITLE_H - XLABEL_H);
        let card = Rect::new(c.x + c.w - CARD_W - 2, c.y + 2, CARD_W, c.h - 4);
        self.draw_trend_card(cv, card, row, t);
        let Some((t, (lo, hi, tick))) = t.and_then(|t| t.scale.map(|s| (t, s))) else {
            chart_grid(cv, g, 5, 4);
            cv.text_in(&f.s, g, 1, "collecting values", DIM);
            return;
        };
        let now = t.pts.back().map_or(0.0, |p| p.t);
        let age = now - t.pts.front().map_or(now, |p| p.t);
        let window = TREND_WINDOWS.iter().copied().find(|&w| w >= age).unwrap_or((age / 3600.0).ceil() * 3600.0);
        chart_grid(cv, g, 5, 1);
        let y_of = |v: f64| g.y + g.h - 1 - ((v - lo) / (hi - lo) * (g.h - 1) as f64).round() as i32;
        let big = lo.abs().max(hi.abs()) as f32;
        let mut v = (lo / tick).ceil() * tick;
        while v <= hi + tick * 1e-6 {
            let y = y_of(v);
            cv.hline(g.x + 1, g.x + g.w - 2, y, GRID_LINE);
            let s = axis_label(v as f32, tick as f32, 0.0, big, row.unit);
            cv.text(&f.s, g.x - 5 - f.s.width(&s), (y + 4).clamp(g.y + 9, g.y + g.h), &s, AXIS_TEXT);
            v += tick;
        }
        for i in 0..=5 {
            let s = if i == 5 { "now".to_string() } else { format!("-{:.0} s", window * (5 - i) as f64 / 5.0) };
            let w = f.s.width(&s);
            let x = g.x + i * (g.w - 1) / 5 - w / 2;
            cv.text(&f.s, x.clamp(g.x, g.x + g.w - w), g.y + g.h + 13, &s, AXIS_TEXT);
        }
        cv.set_clip(Rect::new(g.x + 1, g.y + 1, g.w - 2, g.h - 2));
        let x_of = |tp: f64| g.x + g.w - 1 - ((now - tp) / window * (g.w - 1) as f64).round() as i32;
        let band = mix(BG, col, 80);
        let mut prev: Option<(i32, i32)> = None;
        for p in &t.pts {
            if !p.v.is_finite() {
                prev = None;
                continue;
            }
            let x = x_of(p.t);
            cv.vline(x, y_of(p.hi as f64), y_of(p.lo as f64), band);
            let q = (x, y_of(p.v as f64));
            if let Some(o) = prev {
                cv.line(o.0, o.1, q.0, q.1, col);
            }
            prev = Some(q);
        }
        cv.no_clip();
    }

    /// The value large, under it largest and smallest of the history; the rows share the card.
    fn draw_trend_card(&self, cv: &mut Canvas, r: Rect, row: &tools::MeasRow, t: Option<&Trend>) {
        let f = self.fonts();
        cv.round(r, 5, BAR, 255);
        // the close button may be over the top right corner of the card
        cv.text(&f.s, r.x + 8, r.y + 18, "Now", DIM);
        let now = row.fmt4(row.v);
        let font = fit_font(&[&f.xl, &f.lb, &f.mb], r.w - 12, std::iter::once(now.as_str()));
        let body = r.h - 40;
        let col = self.wave_color(row.src);
        cv.text(font, r.x + r.w - 6 - font.width(&now), r.y + 40 + body / 3 - 4, &now, col);
        let (lo, hi) = t
            .and_then(|t| t.range)
            .map_or(("----".to_string(), "----".to_string()), |(a, b)| (row.fmt4(a), row.fmt4(b)));
        for (k, (l, v)) in [("Max", hi), ("Min", lo)].iter().enumerate() {
            let y = r.y + 40 + body / 3 + (k as i32 + 1) * (body * 2 / 3) / 3 + 4;
            cv.text(&f.s, r.x + 8, y, l, DIM);
            cv.text(&f.sb, r.x + r.w - 8 - f.sb.width(v), y, v, LABEL);
        }
    }

    /// A tap on a chart: its next measurement.
    fn trend_tap(&mut self, x: i32, y: i32) {
        let n = self.meas_rows().len();
        let charts = self.trend_charts();
        if n == 0 {
            return;
        }
        for (c, &(k, row)) in Self::chart_rects(self.pane_rect(), charts.len()).iter().zip(&charts) {
            if c.contains(x, y) {
                self.m.trend_sel[k] = (row + 1) % n + 2;
            }
        }
    }

    // ------------------------------------------------------------------------------------------------------------
    // the pane as a whole

    /// The content of the pane.
    pub(super) fn draw_pane(&self, cv: &mut Canvas) {
        let r = self.pane_rect();
        cv.fill(r, BG);
        cv.hline(r.x, r.x + r.w - 1, r.y, LINE);
        match self.m.pane {
            tables::PANE_SPECTRUM => self.draw_spectrum(cv, r, true),
            _ => {
                cv.set_clip(r);
                match self.m.pane {
                    tables::PANE_RESULTS => self.draw_results_pane(cv, r),
                    tables::PANE_EVENTS => self.draw_pane_events(cv, r),
                    tables::PANE_TREND => self.draw_trends(cv, r),
                    tables::PANE_BUS => self.draw_pane_bus(cv, r),
                    _ => {}
                }
                cv.no_clip();
            }
        }
    }

    /// A tap on the pane, or on the sort button of the full view; true when it was used.
    pub(super) fn pane_tap(&mut self, x: i32, y: i32) -> bool {
        if self.m.view == AView::Spectrum && !self.xy && self.azr.spec.is_some() {
            let b = self.spec_box(self.area, false);
            if b.sort.contains(x, y) {
                self.sort_toggle();
                return true;
            }
        }
        if self.pane_h() == 0 {
            return false;
        }
        let r = self.pane_rect();
        if Self::pane_close(r).contains(x, y) {
            self.m.pane = 0;
            self.m.split_pane = 0;
            // Split with no number pane is the two stacked traces
            if self.m.density == Density::Split && self.m.win_layout == WinLayout::One {
                self.m.win_layout = WinLayout::Rows;
                self.spread_wins();
            }
            return true;
        }
        if !r.contains(x, y) {
            return false;
        }
        match self.m.pane {
            tables::PANE_SPECTRUM => {
                let b = self.spec_box(r, true);
                if b.sort.contains(x, y) {
                    self.sort_toggle();
                } else if b.title.contains(x, y) {
                    self.toggle_menu(MenuId::Spectrum);
                }
            }
            tables::PANE_TREND => self.trend_tap(x, y),
            _ => {}
        }
        true
    }
}
