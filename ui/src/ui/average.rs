// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{drawpipe::average, picture::FirstDraw};

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

pub(super) fn blend_record_average(ui: &Ui, rec: &mut Record, first: FirstDraw) {
    if ui.m.acq != AcqMode::Average {
        return;
    }
    if ui.src.is_hw() {
        average::record_fallback(&ui.m, rec);
    }
    if rec.raw.as_ref().is_some_and(|r| r.frame.hw_avg) || first.waiting() {
        return;
    }
    if let Some(prev) = &ui.last[0] {
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
}

pub(super) fn prepare_batch_average(ui: &mut Ui, rec: &mut Record, average: &average::Output, first: FirstDraw) {
    let previous = ui.last[0].as_ref().filter(|_| !first.waiting());
    average.apply(previous, &mut ui.batch_picture.average_screen);
    std::mem::swap(&mut ui.batch_picture.average_screen, &mut rec.ch);
}

pub(super) fn store_average_screen(ui: &mut Ui, batched: bool) {
    if batched {
        if let Some(previous) = &mut ui.last[0] {
            ui.batch_picture.average_screen = std::mem::take(&mut previous.ch);
        }
    }
}

pub(super) fn remember_after_draw(ui: &mut Ui, rec: &Record, hw_seg: bool, batched: bool) {
    if batched {
        remember_average(ui, rec);
        ui.hist_pos = None;
        ui.mask_test(rec);
    } else {
        ui.batch_picture.average_history_channels.fill(false);
        deep_draw::remember_record(ui, rec, hw_seg);
    }
}

fn remember_average(ui: &mut Ui, rec: &Record) {
    use crate::board::record::HW_POINTS;
    let seen = &mut ui.batch_picture.average_history_channels;
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

pub(super) fn prepare(ui: &mut Ui, record: &mut Option<crate::board::Acquired>, first: FirstDraw) {
    if let Some((rec, _, _, Some(output))) = record {
        prepare_batch_average(ui, rec, output, first);
    }
}

pub(super) fn filtered(
    ui: &mut Ui,
    rec: &mut Record,
    composition: &mut step_draw::Composition,
    batched: bool,
    eligibility: PictureEligibility,
) {
    if batched {
        // The unshown batch accounts for all its records when its output drops.
        composition.filtered -= usize::from(eligibility == PictureEligibility::Drawable);
        std::mem::swap(&mut ui.batch_picture.average_screen, &mut rec.ch);
    }
}

pub(super) fn draw_average(
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
