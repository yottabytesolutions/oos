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
