// SPDX-License-Identifier: GPL-2.0-or-later
//! UI-clock peak tracking after accepted spectrum results. Raw measurements remain immediate.

use super::*;

/// Sort raw detections; the display table observes sort changes at its next refresh or tick.
pub(in crate::ui) fn peaks_sort(ui: &mut Ui) {
    if ui.m.spec_sort == 0 {
        ui.azr.peaks.sort_by(|a, b| a.f.total_cmp(&b.f));
    } else {
        ui.azr.peaks.sort_by(|a, b| b.dbv.total_cmp(&a.dbv));
    }
}

pub(in crate::ui) fn track_peaks(ui: &mut Ui) {
    let Some(sp) = &ui.azr.spec else {
        ui.azr.work.peak_table = peaks::Table::default();
        return;
    };
    ui.azr.work.peak_table.refresh(
        &ui.azr.peaks,
        ui.now,
        sp,
        (ui.m.spec_src, ui.m.spec_win, ui.m.spec_trace),
        ui.m.spec_sort != 0,
    );
}

/// The spectrum of the source from the record: the view, or `pane` beside the waveform.
pub(in crate::ui) fn spectrum_update(ui: &mut Ui, rec: &Record, pane: bool) {
    let id = ui.m.spec_src;
    let Some(s) = ui.wave_samples(rec, id) else {
        ui.azr.spec = None;
        ui.azr.peaks.clear();
        ui.track_peaks();
        return;
    };
    let (v, dt) = (s.v, s.dt);
    let f1 = ui.wave_freq(rec, id);
    let (mut center, mut span) = (ui.m.spec_center, ui.m.spec_span);
    if window::follows(ui) {
        // no frequency this record: the span stays
        ui.azr.spec_stop =
            spectrum::auto_stop(f1, PANE_HARMS[ui.m.spec_harm.min(PANE_HARMS.len() - 1)], ui.azr.spec_stop);
        (center, span) = (ui.azr.spec_stop / 2.0, ui.azr.spec_stop);
    }
    let area = if pane { ui.pane_rect() } else { ui.area };
    // Auto RBW: the finest the record allows (window ENBW / record time)
    let finest = spectrum::enbw(ui.m.spec_win) / (v.len().max(1) as f64 * dt);
    let s = spectrum::Settings {
        center,
        span,
        rbw: if ui.m.spec_rbw > 0.0 { ui.m.spec_rbw } else { finest },
        window: ui.m.spec_win,
        averaging: 16,
        trace: [spectrum::Trace::Normal, spectrum::Trace::Average, spectrum::Trace::MaxHold, spectrum::Trace::MinHold]
            [ui.m.spec_trace.min(3)],
        detector: spectrum::Detector::Peak,
        points: ui.spec_box(area, pane).plot.w.max(100) as usize,
        ref_level_dbv: ui.m.spec_ref as f64,
    };
    let sp = spectrum::analyze(&v, dt, &s, &mut ui.azr.spec_avg);
    ui.azr.peaks = spectrum::rank_lines(&sp, Some(f1), panes::PEAKS_MAX);
    peaks_sort(ui);

    if ui.m.spec_wf && !pane {
        ui.azr.wf.get_or_insert_with(|| spectrum::Waterfall::new(120)).push(&sp);
    }
    ui.azr.dist = if ui.m.spec_dist && !pane { spectrum::distortion(&v, dt, None, 10) } else { None };
    if let Some(l) = &ui.azr.limit {
        ui.azr.limit_n += 1;
        if let Some(w) = spectrum::limit_check(&sp, l) {
            ui.azr.limit_over += 1;
            if ui.azr.limit_worst.is_none_or(|x| w.1 > x.1) {
                ui.azr.limit_worst = Some(w);
            }
        }
    }
    ui.azr.spec = Some(sp);
    ui.track_peaks();
}
