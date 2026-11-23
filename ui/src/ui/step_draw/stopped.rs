// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(in crate::ui) fn stopped(ui: &mut Ui) {
    if ui.m.run == RunState::Stop && ui.stop_final.is_none() {
        if let Some(pipe) = drawpipe::pipe() {
            ui.batch_picture.owner.retire_inputs(pipe, false);
        }
    }
    let average = ui.readers_use_average();
    if !average {
        ui.stop_replay();
    }
    ui.ph.clear();
    ui.phz.clear();
    if average {
        if let Some(record) = ui.last[0].take() {
            deep_draw::draw_record_weighted(ui, &record, 24 * 6);
            ui.last[0] = Some(record);
        }
    } else {
        draw_native_capture(ui);
    }
}

fn draw_native_capture(ui: &mut Ui) {
    let mainv = (ui.m.time_per_div(), ui.m.delay);
    let zv = (tscales()[ui.m.zscale], ui.m.zdelay);
    let zoom_model = ui.zoom_display_model();
    let detail = ui
        .stopped_capture()
        .filter(|_| ui.stop_picture().is_some())
        .map_or_else(|| ui.captured_view(zv), |(capture, _)| ui.captured_record_view(capture, zv));
    // the view read again as pairs while the raw record of the STOP is the one measured: the pairs are drawn
    let shown = ui.stop_picture().cloned().or_else(|| ui.last[0].clone());
    if let Some(rec) = shown {
        let zw = ui.z_weights(&rec);
        for c in 0..NCH {
            if ui.m.ch[c].on && !rec.ch[c].is_empty() && ui.m.z_src != Some(c) {
                let shifted_main = ui.deskew_channel_view(&rec, c, mainv);
                let main = shifted_main.as_deref().unwrap_or(&rec);
                ui.ph.draw_ex(
                    c,
                    &ui.m,
                    mainv,
                    main.t0 - ui.m.deskew_ch[c],
                    main.dt,
                    &main.ch[c],
                    24 * 6,
                    if shifted_main.is_some() { None } else { zw.as_deref() },
                    main.pairs,
                );
                if ui.m.zoom {
                    let shifted = ui.deskew_channel_view(&rec, c, zv);
                    let z = shifted.as_deref().or(detail.as_deref()).unwrap_or(&rec);
                    ui.phz.draw_ex(
                        c,
                        &zoom_model,
                        zv,
                        z.t0 - ui.m.deskew_ch[c],
                        z.dt,
                        &z.ch[c],
                        24 * 6,
                        if shifted.is_some() || detail.is_some() { None } else { zw.as_deref() },
                        z.pairs,
                    );
                }
            }
        }
    }
}
