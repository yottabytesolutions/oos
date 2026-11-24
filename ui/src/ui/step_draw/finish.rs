// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(in crate::ui) fn finish(ui: &mut Ui, mut parts: [Duration; 5], records: usize) {
    let started = Instant::now();
    // brightest pixel per channel: the trace brightness is graded relative to it
    // (follow upwards at once, downwards slowly: no flicker)
    let (full, fullz) = if ui.batch_picture.replacement_pending { (false, false) } else { ui.grade_refs() };
    ui.prof_add(3, started.elapsed());
    // long or infinite persistence: halve the maps before the counts saturate (keeps the density ratios)
    if full {
        ui.ph.halve();
        ui.vmax.iter_mut().for_each(|v| *v = (*v / 2).max(1));
        ui.vmax_cg = (ui.vmax_cg / 2).max(1);
    }
    parts[4] += started.elapsed();
    change::finish_step(ui);
    let halve_zoom = Instant::now();
    if fullz {
        ui.phz.halve();
        ui.vmaxz.iter_mut().for_each(|v| *v = (*v / 2).max(1));
        ui.vmaxz_cg = (ui.vmaxz_cg / 2).max(1);
    }
    parts[4] += halve_zoom.elapsed();

    ui.ph.publish();
    ui.phz.publish();
    #[cfg(feature = "private-picture-counts")]
    crate::drawpipe::counters().timing.picture_counts().published();
    let timing = change::timing_mut(ui);
    timing.draw_step(parts);
    if crate::drawpipe::counters().diagnostics_enabled() {
        timing.picture_frames(records);
    }
}
