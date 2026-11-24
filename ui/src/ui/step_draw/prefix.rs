// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::picture::deposit::standard::{add_frame_band, fast_plan};

/// Draw the decoded batch fallback up to its newest record. Place in parallel (trigger, columns or
/// screen record, statistics of each frame), then the maps filled in bands, channel by band on several cores.
/// Returns the frames that counted as triggered.
pub(in crate::ui) fn draw_queued_prefix(ui: &mut Ui, frames: Vec<std::sync::Arc<crate::board::Frame>>) -> usize {
    let (w, h) = (ui.ph.w, ui.ph.h);
    let m = &ui.m;
    // the statistics of every frame for the voltage measurements, once the measurements of the record have set
    // up their rows
    let rows_ready = ui.mstat.len() == m.meas.len() + m.meas2.len();
    let fold: [bool; NCH] = std::array::from_fn(|c| {
        rows_ready
            && m.ch[c].on
            && m.meas.iter().any(|&(k, mc)| mc == WaveId::Ch(c) && crate::fastdraw::Fold::covers(k))
    });
    let t0 = std::time::Instant::now();
    let placed = super::place_records(&frames, |f| fast_plan(m, w, h, f, fold));
    let t1 = std::time::Instant::now();
    let mut trig = 0;
    for (p, f) in placed.iter().zip(&frames) {
        trig += p.triggered as usize;
        if let Some(outcome) = p.plan.outcome() {
            crate::drawpipe::Stats { edges: &mut ui.fast_edges, off: &mut ui.fast_off }.record(outcome);
        }
        // as the deposit workers (`framestats::fold_frame`): nothing of live-compression pairs (the raw newest
        // frame of the batch counts), of other min/max pairs only what pairs give (Max, Min, Pk-pk)
        if rows_ready && !crate::compress::live(f) {
            for (i, &(k, src)) in ui.m.meas.iter().enumerate() {
                let Some(c) = src.ch() else { continue };
                if f.envelope && crate::framestats::needs_samples(k) {
                    continue;
                }
                if let Some(v) = p.plan.fold(c).and_then(|f| f.get(k)).filter(|v| v.is_finite()) {
                    ui.mstat[i].add(v);
                }
            }
        }
    }
    if ui.ph.is_wide() || placed.iter().any(|p| p.plan.format() == crate::picture::deposit::count::CountFormat::Wide) {
        for (p, f) in placed.iter().zip(&frames) {
            crate::picture::deposit::standard::wide_frame(&mut ui.ph, f, &p.plan);
        }
        return trig;
    }
    draw_queued_bands(ui, &frames, &placed, (t0, t1));
    trig
}

fn draw_queued_bands(
    ui: &mut Ui,
    frames: &[std::sync::Arc<crate::board::Frame>],
    placed: &[crate::picture::deposit::standard::FastFrame],
    (t0, t1): (std::time::Instant, std::time::Instant),
) {
    let (w, h) = (ui.ph.w, ui.ph.h);
    let m = &ui.m;
    // the maps: each channel's frames collected as differences (`fastdraw::diff_span`), in bands of columns so one
    // channel alone also spreads over the cores, then summed into the map once
    let on: Vec<usize> = (0..NCH).filter(|&c| m.ch[c].on).collect();
    for &c in &on {
        if ui.acc_t[c].len() != w * (h + 1) {
            ui.acc_t[c] = vec![0; w * (h + 1)];
        }
    }
    let bands = if frames.len() < 2 || on.is_empty() { 1 } else { (crate::fastdraw::helpers() + 1).div_ceil(on.len()) };
    let band_cols = w.div_ceil(bands).max(1);
    let mut jobs: Vec<(usize, usize, &mut [u16])> = Vec::new();
    for (c, acc) in ui.acc_t.iter_mut().enumerate() {
        if on.contains(&c) {
            jobs.extend(acc.chunks_mut(band_cols * (h + 1)).enumerate().map(|(b, band)| (c, b * band_cols, band)));
        }
    }
    crate::fastdraw::par_each(jobs, |(c, c_lo, band)| {
        for (f, p) in frames.iter().zip(placed) {
            add_frame_band(c, band, c_lo, f, &p.plan);
        }
    });
    // the rows the frames touched, per channel: only those are added (a trace covers a part of the height)
    let rows: [(usize, usize); NCH] = std::array::from_fn(|c| {
        placed.iter().fold((usize::MAX, 0), |(a, b), p| {
            let (x, y) = p.plan.rows(c);
            (a.min(x), b.max(y))
        })
    });
    let t2 = std::time::Instant::now();
    ui.ph.fold_rows(&mut ui.acc_t, &on, band_cols, rows);
    let (a, b) = ((t1 - t0).as_secs_f32() * 1e3, t1.elapsed().as_secs_f32() * 1e3);
    ui.t_fast_ab.0 += 0.05 * (a - ui.t_fast_ab.0);
    ui.t_fast_ab.1 += 0.05 * (b - ui.t_fast_ab.1);
    ui.t_fast_fold += 0.05 * (t2.elapsed().as_secs_f32() * 1e3 - ui.t_fast_fold);
    ui.fast_n += 0.05 * (frames.len() as f32 - ui.fast_n);
}
