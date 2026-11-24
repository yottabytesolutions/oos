// SPDX-License-Identifier: GPL-2.0-or-later

//! Trigger placement for decoded and DMA batch records.

use super::*;
use std::time::Instant;

pub(super) fn count_short_window(f: &crate::board::Frame, (first, end): (f64, f64), ti: f64, points: usize) {
    if f.trigger_anchor.cropped() && (ti + first / f.dt < 0.0 || ti + end / f.dt > points as f64) {
        fallback::short_play_window();
    }
}

pub(crate) fn batch_trigger(
    m: &Model,
    f: &crate::board::Frame,
    proj: &Projection,
    meta: &FrameTiming,
    (source, route): (&SamplesRef<'_>, Route),
    refine_time: Option<&mut Duration>,
) -> (f64, kernel::TrigOutcome) {
    if let Some(ti) = hardware_position(m, f) {
        #[cfg(test)]
        crate::trignear::fixture::placed(f, ti);
        return (ti, kernel::TrigOutcome::Hardware);
    }
    #[cfg(test)]
    if crate::trignear::fixture::bypass() {
        return (meta.anchor, kernel::TrigOutcome::Hardware);
    }
    let col = if route == Route::Pairs {
        crate::compress::smooth_len(proj.x_per_sample)
    } else {
        (1.0 / proj.x_per_sample).floor().clamp(1.0, 256.0) as usize
    };
    let result = kernel::refine(meta, col, || {
        timed_trigger(refine_time, || refine_batch(m, f, proj.time_per_div, meta, source, col))
    });
    #[cfg(test)]
    crate::trignear::fixture::placed(f, result.0);
    result
}

pub(super) fn timed_trigger<T>(time: Option<&mut Duration>, mut search: impl FnMut() -> T) -> T {
    let started = time.as_ref().map(|_| Instant::now());
    let result = fixture_stage!(Trigger, search());
    if let Some((started, time)) = started.zip(time) {
        *time = started.elapsed();
    }
    result
}

fn refine_batch(
    m: &Model,
    f: &crate::board::Frame,
    time_per_div: f64,
    meta: &FrameTiming,
    source: &SamplesRef<'_>,
    col: usize,
) -> crate::trignear::SearchResult {
    match source {
        SamplesRef::Codes(_) => Cause::TriggerFromCodes,
        SamplesRef::Volts(_) => Cause::TriggerFromVolts,
    }
    .mark();
    let level = crate::board::sw_trig_level_of(m, source.len(), |i| source.at(i));
    let half = (time_per_div * HDIV as f64 / 2.0 / f.dt) as usize;
    let t = meta.trigger;
    let trigger = crate::fastdraw::EdgeSearch { level, hysteresis: t.hysteresis, rising: t.rising, either: t.either };
    let search = (meta.anchor, half, col);
    // A live crop has room for the near search. Filtered comparators and full-rate anchors also stay local.
    if f.trigger_anchor.cropped()
        || matches!(m.trig_coupling(), TrigCoup::Ac | TrigCoup::LfReject)
        || (f.trigger_anchor.at_adc_rate() && m.trig_type == TrigType::Edge)
    {
        crate::trignear::edge_near_from(source, (0, source.len()), search, trigger)
    } else {
        crate::trignear::edge_smooth_near_with(source, search, trigger, &|_| true, None)
    }
}
