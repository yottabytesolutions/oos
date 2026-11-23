// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{
    drawpipe::{average, Lost},
    picture::FirstDraw,
};

pub(super) fn batch_average_ok(ui: &Ui) -> bool {
    average::software(&ui.m)
        && ui.m.avg_count != 0
        && ui.m.n_on() != 0
        && ui.m.trig_type == TrigType::Edge
        && !ui.m.trig_on_la()
        && !ui.m.la_on()
        && !ui.m.mask_on
        && !ui.m.zones.iter().any(|z| z.usable())
}

#[derive(Default)]
pub(super) struct State {
    screen: [Vec<f32>; NCH],
    grid: Option<Grid>,
    history_channels: Option<[bool; NCH]>,
}

pub(super) fn reset_history_channels(state: &mut State) {
    state.history_channels = None;
}

#[derive(PartialEq)]
struct Grid {
    time: (f64, f64, bool),
    main: (f32, f32),
    zoom: Option<(f32, f32)>,
    depth: usize,
    channels: [bool; NCH],
    trigger: (TrigType, oos_dsp::trigger::Trigger, TrigCoup, Option<u8>, bool),
}

impl Grid {
    fn of(m: &Model, rec: &Record) -> Self {
        Self {
            time: (rec.t0, rec.dt, rec.pairs),
            main: (m.time_per_div(), m.delay),
            zoom: m.zoom.then(|| (TIME_SCALES[m.zscale], m.zdelay)),
            depth: m.mdepth,
            channels: m.ch.each_ref().map(|c| c.on),
            trigger: (m.trig_type, m.trigger(), m.trig_coupling(), m.trig_line(), m.trig_nrej),
        }
    }

    fn matches(&self, rec: &Record) -> bool {
        self.time == (rec.t0, rec.dt, rec.pairs)
    }
}

pub(super) fn current_grid(ui: &Ui, record: &Record) -> bool {
    ui.batch_picture.average.grid.as_ref().is_some_and(|grid| *grid == Grid::of(&ui.m, record))
}

fn previous<'a>(ui: &'a Ui, grid: &Grid, first: FirstDraw) -> Option<&'a Record> {
    ui.last[0]
        .as_ref()
        .filter(|prev| !first.waiting() && ui.batch_picture.average.grid.as_ref() == Some(grid) && grid.matches(prev))
}

pub(super) fn blend_record_average(ui: &mut Ui, rec: &mut Record, first: FirstDraw) {
    let grid = Grid::of(&ui.m, rec);
    if ui.m.acq != AcqMode::Average {
        ui.batch_picture.average.grid = Some(grid);
        return;
    }
    let hw_avg = rec.raw.as_ref().is_some_and(|raw| raw.frame.hw_avg);
    let seed = if hw_avg { None } else { previous(ui, &grid, first) };
    if let Some(prev) = seed {
        let a = 1.0 / ui.m.avg_count as f32;
        for (values, previous) in rec.ch.iter_mut().zip(&prev.ch) {
            if values.len() == previous.len() {
                for (x, p) in values.iter_mut().zip(previous) {
                    if p.is_finite() {
                        *x = p + (*x - p) * a;
                    }
                }
            }
        }
    }
    ui.batch_picture.average.grid = Some(grid);
}

pub(super) fn count_record_fallback(ui: &Ui, rec: &Record) {
    use crate::drawpipe::metrics::fallback::{self, Cause};
    if !ui.src.is_hw() || !average::software(&ui.m) {
        return;
    }
    let m = &ui.m;
    let frame = rec.raw.as_ref().map(|raw| &raw.frame);
    let cause = match () {
        _ if frame.is_some_and(|frame| frame.roll) => Cause::AverageRoll,
        _ if frame.is_some_and(|frame| frame.scan) => Cause::AverageScan,
        _ if m.trig_mode == TrigMode::Single => Cause::AverageSingle,
        _ if m.segments != 0 => Cause::AverageSegments,
        _ if m.la_on() || m.trig_on_la() => Cause::AverageLa,
        _ if m.trig_type != TrigType::Edge => Cause::AverageTrigger,
        _ if m.mask_on => Cause::AverageMask,
        _ if m.zones.iter().any(|z| z.usable()) => Cause::AverageZone,
        _ if ui.xy => Cause::AverageXy,
        _ if m.z_src.is_some() => Cause::AverageZ,
        _ if m.ch.iter().zip(m.deskew_ch).any(|(ch, shift)| ch.on && shift != 0.0) => Cause::AverageDeskew,
        _ if frame.is_some_and(|frame| frame.envelope) => Cause::AveragePairs,
        _ => Cause::DirectRecord,
    };
    fallback::direct(|| cause.mark());
}

fn remember_average(ui: &mut Ui, rec: &Record) {
    use crate::board::record::HW_POINTS;
    let seen = ui.batch_picture.average.history_channels.get_or_insert([false; NCH]);
    // Size existing slots when a channel first appears, before rotating them
    // through several batches with different active channel masks.
    for (c, values) in rec.ch.iter().enumerate() {
        if !seen[c] && !values.is_empty() {
            seen[c] = true;
            for record in &mut ui.history {
                let slot = &mut record.ch[c];
                slot.reserve_exact(HW_POINTS.saturating_sub(slot.len()));
            }
        }
    }
    let mut channels = if ui.history.len() >= HISTORY {
        ui.history.pop_front().expect("the full history has an oldest record").ch
    } else {
        seen.map(|on| Vec::with_capacity(usize::from(on) * HW_POINTS))
    };
    for (stored, input) in channels.iter_mut().zip(&rec.ch) {
        stored.clone_from(input);
    }
    ui.history.push_back(Record { ch: channels, raw: None, la: None, ..*rec });
}

pub(super) fn consume_batch(
    ui: &mut Ui,
    (mut rec, eligibility, mut output): (Record, PictureEligibility, average::Output),
    composition: &mut step_draw::Composition,
    first: &mut FirstDraw,
    taps: &mut deep_draw::ReaderTaps,
) -> Option<(usize, std::time::Duration)> {
    let mut screen = std::mem::take(&mut ui.batch_picture.average.screen);
    let grid = Grid::of(&ui.m, &rec);
    output.apply(previous(ui, &grid, *first), &mut screen);
    rec.ch = screen;
    if composition.filter_record(ui, &rec, eligibility, first) {
        composition.filtered -= usize::from(eligibility == PictureEligibility::Drawable);
        ui.batch_picture.average.screen = rec.ch;
        output.set_loss(Lost::Average);
        return None;
    }

    ui.m.status = if rec.triggered { TrigStatus::Triggered } else { TrigStatus::Auto };
    let started = std::time::Instant::now();
    let drawn = draw_average(ui, composition, &rec, output, first);
    let elapsed = started.elapsed();
    remember_average(ui, &rec);
    taps.next_record(ui, true);
    if let Some(previous) = ui.last[0].replace(rec) {
        ui.batch_picture.average.screen = previous.ch;
    }
    ui.batch_picture.average.grid = Some(grid);
    ui.hist_pos = None;
    ui.cleared = false;
    ui.records += 1;
    Some((drawn, elapsed))
}

fn draw_average(
    ui: &mut Ui,
    composition: &mut step_draw::Composition,
    rec: &Record,
    output: crate::drawpipe::average::Output,
    first: &mut FirstDraw,
) -> usize {
    if first.begin() {
        step_draw::clear_maps(ui);
    }
    deep_draw::draw_record(ui, rec);
    let (records, read_at) = output.shown();
    #[cfg(feature = "private-picture-counts")]
    if composition.merged == 0 {
        composition.oldest = read_at;
    } else {
        composition.oldest = composition.oldest.zip(read_at).map(|(a, b)| a.min(b));
    }
    composition.merged += records as u64;
    composition.newest = composition.newest.max(read_at);
    if let Some(raw) = &rec.raw {
        crate::pacing::drawn(raw.frame.cfg, records as u64);
    }
    composition.deposit += records;
    records
}

#[cfg(test)]
mod tests;
