// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drawpipe::reuse::EdgeInputs;

pub(super) fn acquire(ui: &mut Ui, elapsed: f64) -> (Option<Record>, Option<EdgeInputs>) {
    // Capture before conversion and compare again at drawing. Intervening routing
    // or settings changes cannot silently reuse a different view.
    let input = if ui.fast_ok() { EdgeInputs::of(&ui.m) } else { None };
    (ui.acquire_bus_candidate(elapsed), input)
}

pub(super) fn draw(
    ui: &mut Ui,
    f: &crate::board::Frame,
    rec: &Record,
    interp: crate::sparse::Interp,
    input: Option<EdgeInputs>,
) {
    let stats = crate::drawpipe::Stats { edges: &mut ui.fast_edges, off: &mut ui.fast_off };
    crate::drawpipe::draw_frame_reusing(&mut ui.ph, &ui.m, f, interp, Some(stats), input.map(|input| (rec, input)));
}

pub(super) fn newest(ui: &mut Ui, waveform: Option<std::sync::Arc<crate::board::Frame>>) -> Option<u64> {
    let frame = waveform?;
    let seq = frame.seq;
    if let Source::Hw { shared, .. } = &ui.src {
        let mut g = oos_hal::sync::lock(shared);
        g.frame = frame.clone();
        g.queue.push_back(frame);
    }
    Some(seq)
}

pub(super) fn record(
    ui: &mut Ui,
    rec: &Record,
    interp: crate::sparse::Interp,
    input: Option<EdgeInputs>,
    deposited: bool,
) -> (usize, usize) {
    if deposited {
        return (0, 0);
    }
    if let Some(raw) = &rec.raw {
        crate::pacing::drawn(raw.frame.cfg, 1);
    }
    if let Some(raw) = rec.raw.as_ref().filter(|r| ui.fast_ok() && !crate::deepmeas::deep_raw_frame(&r.frame)) {
        draw(ui, &raw.frame, rec, interp, input);
        (1, 0)
    } else {
        draw_record(ui, rec);
        (0, 1)
    }
}

fn draw_record(ui: &mut Ui, rec: &Record) {
    let mainv = (ui.m.time_per_div(), ui.m.delay);
    let zv = (tscales()[ui.m.zscale], ui.m.zdelay);
    let zoom_model = ui.zoom_display_model();
    let detail = ui.captured_record_view(rec, zv);
    let zw = ui.z_weights(rec);
    for c in 0..NCH {
        if ui.m.ch[c].on && !rec.ch[c].is_empty() && ui.m.z_src != Some(c) {
            let shifted_main = ui.deskew_channel_view(rec, c, mainv);
            let main = shifted_main.as_deref().unwrap_or(rec);
            ui.ph.draw_ex(
                c,
                &ui.m,
                mainv,
                main.t0 - ui.m.deskew_ch[c],
                main.dt,
                &main.ch[c],
                24,
                if shifted_main.is_some() { None } else { zw.as_deref() },
                main.pairs,
            );
            if ui.m.zoom {
                let shifted = ui.deskew_channel_view(rec, c, zv);
                let z = shifted.as_deref().or(detail.as_deref()).unwrap_or(rec);
                ui.phz.draw_ex(
                    c,
                    &zoom_model,
                    zv,
                    z.t0 - ui.m.deskew_ch[c],
                    z.dt,
                    &z.ch[c],
                    24,
                    if shifted.is_some() || detail.is_some() { None } else { zw.as_deref() },
                    z.pairs,
                );
            }
        }
    }
}
