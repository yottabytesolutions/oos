// SPDX-License-Identifier: GPL-2.0-or-later
//! Spectrum layout and numbered peak readouts.

use super::*;

/// Height of the waterfall under the spectrum.
pub(in crate::ui) fn spec_wf_h(ui: &Ui, a: Rect, pane: bool) -> i32 {
    if ui.m.spec_wf && !pane {
        a.h * 2 / 5
    } else {
        0
    }
}

/// Layout of the spectrum in `a`. In the full view the list starts under the overflow button, in the pane the close
/// button sits beside the sort button.
pub(in crate::ui) fn spec_box(ui: &Ui, a: Rect, pane: bool) -> SpecBox {
    let col_x = a.x + a.w - PEAK_W;
    let title = Rect::new(a.x + AXIS_W, a.y, a.w - AXIS_W - PEAK_W - 8, TITLE_H);
    let plot = Rect::new(title.x, a.y + TITLE_H, title.w, a.h - ui.spec_wf_h(a, pane) - TITLE_H - XLABEL_H);
    // Analysis views have no zoom button. Reserve only the actual corner controls.
    let below = (ui.full_home().1.bottom().max(ui.view_btn().bottom()) + 2).max(a.y + 46);
    let sort = if pane { Rect::new(col_x, a.y + 2, PEAK_W - 50, 36) } else { Rect::new(col_x, below, PEAK_W - 6, 36) };
    let y = sort.y + sort.h + 2;
    SpecBox { title, plot, sort, list: Rect::new(col_x, y, PEAK_W - 6, a.y + a.h - y) }
}

/// Lines shown on the trace and in the list: the numbers must match, so no more than the list has room for.
pub(super) fn peaks_shown(_ui: &Ui, b: &SpecBox) -> usize {
    PEAKS_MAX.min((b.list.h / 18).max(0) as usize)
}

fn sort_name(ui: &Ui) -> &'static str {
    if ui.m.spec_sort == 0 {
        "Peaks by frequency"
    } else {
        "Peaks by level"
    }
}

pub(in crate::ui) fn sort_toggle(ui: &mut Ui) {
    ui.m.spec_sort = 1 - ui.m.spec_sort.min(1);
    ui.peaks_sort();
}

/// Title, grid and axis labels.
pub(in crate::ui) fn draw_spec_frame(ui: &Ui, cv: &mut Canvas, b: &SpecBox, sp: &spectrum::Spectrum) {
    let f = ui.fonts();
    let g = b.plot;
    let src = ui.m.spec_src;
    let name = match src {
        WaveId::Ch(c) => ui.ch_name(c),
        WaveId::Ref(i) => format!("R{}", i + 1),
    };
    let x = cv.text(&f.sb, b.title.x, b.title.y + 14, &name, ui.wave_color(src));
    let mut t = format!(
        "{:.0} dB/div  \u{b7}  Ref {:.0} dBV  \u{b7}  RBW {}",
        DB_PER_DIV,
        ui.m.spec_ref,
        si_short(sp.rbw, "Hz")
    );
    if ui.azr.limit.is_some() {
        t.push_str(&format!("  \u{b7}  Limit: {} of {} over", ui.azr.limit_over, ui.azr.limit_n));
    }
    cv.text(&f.s, x + 10, b.title.y + 14, &t, LABEL);
    chart_grid(cv, g, HDIV as i32, VDIV as i32);
    // every division while the labels have room
    let every = if g.h as f32 / VDIV < (f.s.size + 2) as f32 { 2 } else { 1 };
    for j in (0..=VDIV as i32).step_by(every) {
        let s = format!("{:.0}", ui.m.spec_ref - j as f32 * DB_PER_DIV);
        let y = (g.y + j * (g.h - 1) / VDIV as i32 + 4).clamp(g.y + 9, g.y + g.h);
        cv.text(&f.s, g.x - 5 - f.s.width(&s), y, &s, AXIS_TEXT);
    }
    let (f0, f1) = (sp.f0, sp.freq(sp.dbv.len().saturating_sub(1)));
    // from the right, so the end of the band always has its label; one that would touch the last is left out
    let mut free = g.x + g.w;
    for i in (0..=HDIV as i32).rev() {
        let s = si_short(f0 + (f1 - f0) * i as f64 / HDIV as f64, "Hz");
        let w = f.s.width(&s);
        let x = (g.x + i * (g.w - 1) / HDIV as i32 - w / 2).clamp(g.x - AXIS_W + 2, g.x + g.w - w);
        if x + w + 10 <= free || i == HDIV as i32 {
            cv.text(&f.s, x, g.y + g.h + 13, &s, AXIS_TEXT);
            free = x;
        }
    }
}

/// Numbered tags on the trace, the same numbers as in the list. Tags that would collide move up.
pub(in crate::ui) fn draw_spec_marks(ui: &Ui, cv: &mut Canvas, b: &SpecBox, sp: &spectrum::Spectrum, top: f32) {
    let f = ui.fonts();
    let g = b.plot;
    let span = sp.df * (sp.dbv.len().max(2) - 1) as f64;
    if !span.is_finite() || span <= 0.0 {
        return;
    }
    cv.set_clip(Rect::new(g.x, g.y, g.w + 8, g.h));
    let mut placed = [Rect::new(0, 0, 0, 0); PEAKS_MAX];
    for (used, (number, m, held)) in ui.azr.work.peak_table.rows().take(ui.peaks_shown(b)).enumerate() {
        let color = if held { DIM } else { ACCENT };
        let x = g.x + ((m.f - sp.f0) / span * (g.w - 1) as f64).round() as i32;
        let y = (g.y as f32 + (top - m.dbv as f32) / DB_PER_DIV * (g.h as f32 / VDIV)) as i32;
        let mut r = Rect::new(x - 7, (y - 20).max(g.y + 1), 15, 14);
        while placed[..used].iter().any(|p| overlaps(*p, r)) {
            r.y += 15;
        }
        placed[used] = r;
        cv.vline(x, r.y + r.h, y.max(r.y + r.h), color);
        cv.round(r, 3, color, 255);
        cv.text_in(&f.sb, r, 1, &format!("{}", number), DARK_TEXT);
    }
    cv.no_clip();
}

/// Facts beside the peaks in the list: first what the view would otherwise not say.
fn spec_facts(ui: &Ui, sp: &spectrum::Spectrum, pane: bool) -> Vec<(String, String)> {
    let db = |x: f64, u: &str| format!("{:.1} {}", x, u);
    let mut v = Vec::new();
    if ui.azr.limit.is_some() {
        v.push(("Limit".to_string(), format!("{} of {} over", ui.azr.limit_over, ui.azr.limit_n)));
        let worst = ui
            .azr
            .limit_worst
            .map_or_else(|| "-".into(), |(hz, over)| format!("+{:.1} dB {}", over, si_short(hz, "Hz")));
        v.push(("Worst".to_string(), worst));
    }
    if let Some(d) = ui.azr.dist.as_ref().filter(|_| ui.m.spec_dist && !pane) {
        v.push(("f1".to_string(), format!("{} {}", si(d.f1, "Hz"), db(d.a1_dbv, "dBV"))));
        v.push(("THD".to_string(), format!("{} ({:.3} %)", db(d.thd_db, "dB"), d.thd_pct)));
        v.push(("THD+N".to_string(), db(d.thdn_db, "dB")));
        v.push(("SNR".to_string(), db(d.snr_db, "dB")));
        v.push(("SINAD".to_string(), db(d.sinad_db, "dB")));
        v.push(("SFDR".to_string(), db(d.sfdr_dbc, "dBc")));
        v.push(("ENOB".to_string(), format!("{:.2} bit", d.enob)));
    } else if ui.m.spec_dist && !pane {
        v.extend(["f1", "THD", "THD+N", "SNR", "SINAD", "SFDR", "ENOB"].map(|s| (s.into(), "-".into())));
    }
    let ac = spectrum::acpr(sp, ui.m.spec_acpr_w, ui.m.spec_acpr_s);
    let dbn = |x: f64| if x.is_finite() { format!("{:.1} dB", x) } else { "-".into() };
    v.push(("ACPR".to_string(), if ac.ratio.is_finite() { format!("{:.3}", ac.ratio) } else { "-".into() }));
    v.push(("ACPR lo".to_string(), dbn(ac.lower_db)));
    v.push(("ACPR hi".to_string(), dbn(ac.upper_db)));
    v.push(("Noise floor".to_string(), db(sp.floor_dbv, "dBV")));
    let stop = sp.freq(sp.dbv.len().saturating_sub(1));
    v.push(("Band".to_string(), format!("{} to {}", si_short(sp.f0, "Hz"), si_short(stop, "Hz"))));
    v.push(("Window".to_string(), ui.m.spec_win.name().to_string()));
    v.push(("Trace".to_string(), ["Normal", "Average", "Max hold", "Min hold"][ui.m.spec_trace.min(3)].to_string()));
    v
}

/// Reserve all peak rows even during qualification or release; the summary never follows the peak count.
pub(in crate::ui) fn draw_peaks(ui: &Ui, cv: &mut Canvas, b: &SpecBox, sp: &spectrum::Spectrum, pane: bool) {
    let f = ui.fonts();
    btn(cv, b.sort, BTN, BTN_BORDER);
    cv.text_in(
        super::super::fit_font(&[&f.mb, &f.sb, &crate::font::Fonts::get(0).mb], b.sort.w - 8, sort_name(ui)),
        b.sort,
        1,
        sort_name(ui),
        TEXT,
    );
    let r = b.list;
    let n = ui.peaks_shown(b) as i32;
    let facts = spec_facts(ui, sp, pane);
    let layout = PeakLayout::new(r, n, facts.len(), pane);
    let pitch = layout.pitch;
    let top = ui.azr.work.peak_table.rows().take(n as usize).map(|(_, m, _)| m.dbv).fold(f64::MIN, f64::max);
    let strongest = ui.azr.work.peak_table.rows().map(|(_, m, _)| m.dbv).fold(f64::MIN, f64::max);
    for (k, (number, m, held)) in ui.azr.work.peak_table.rows().take(n as usize).enumerate() {
        let color = if held { DIM } else { ACCENT };
        let y = r.y + k as i32 * pitch;
        let row = Rect::new(r.x, y, r.w, pitch);
        if k % 2 == 1 {
            cv.fill(row, ROW_ALT);
        }
        let tag = Rect::new(r.x + 2, y + (pitch.min(20) - 14) / 2, 15, 14);
        cv.round(tag, 3, color, 255);
        cv.text_in(&f.sb, tag, 1, &format!("{}", number), DARK_TEXT);
        let ty = tag.y + 12;
        cv.text(&f.sb, r.x + 22, ty, &si_digits(m.f, "Hz", 5), if held { DIM } else { TEXT });
        let s = format!("{:.1} dBV", m.dbv);
        cv.text(&f.s, r.x + r.w - 4 - f.s.width(&s), ty, &s, if held { DIM } else { LABEL });
        // height over the noise on the scale of the list
        let span = (top - sp.floor_dbv).max(1.0);
        let w = (((m.dbv - sp.floor_dbv) / span).clamp(0.0, 1.0) * (r.w - 26) as f64) as i32;
        if pitch >= 36 {
            let s = if m.dbv == strongest { "strongest".to_string() } else { format!("{:.1} dBc", m.dbv - strongest) };
            cv.text(&f.s, r.x + 22, ty + 15, &s, DIM);
        }
        cv.fill(Rect::new(r.x + 22, y + pitch - 3, w, 2), scale(color, 150));
    }
    if ui.azr.work.peak_table.rows().next().is_none() {
        cv.text(&f.s, r.x + 4, r.y + 14, "No lines above the noise", DIM);
    }
    for (k, (l, v)) in facts.iter().take(layout.facts as usize).enumerate() {
        let y = layout.fact_y(k);
        cv.text(&f.s, r.x + 4, y, l, DIM);
        cv.text(&f.s, r.x + r.w - 4 - f.s.width(v), y, v, LABEL);
    }
}
/// Cursor read-out and peak list beside the table; their rows share the height.
pub(in crate::ui) fn draw_results_side(ui: &Ui, cv: &mut Canvas, s: Rect) {
    let f = ui.fonts();
    let cur = ui.cursor_values();
    let peaks = ui.azr.spec.as_ref().map(|_| &ui.azr.work.peak_table);
    let cur_rows = if cur.is_empty() { 0 } else { 1 + cur.len().div_ceil(2) as i32 };
    let peak_rows = peaks.map_or(0, |_| 1 + PEAKS_MAX.div_ceil(2) as i32);
    let rows = cur_rows + peak_rows;
    if rows == 0 {
        return;
    }
    let pitch = s.h / rows;
    let half = s.w / 2;
    let line = |k: i32| s.y + k * pitch + (pitch + 9) / 2;
    if !cur.is_empty() {
        let mode = match ui.m.cursor {
            CursorMode::Volt => "Volt",
            CursorMode::Track => "Track",
            CursorMode::Xy => "XY",
            _ => "Time",
        };
        cv.text(
            &f.sb,
            s.x,
            line(0),
            &format!(
                "Cursors  \u{b7}  {}  \u{b7}  {}",
                mode,
                ui.cursor_badge().map_or("No source".into(), |(label, _)| label)
            ),
            TEXT,
        );
        for (i, (l, v)) in cur.iter().enumerate() {
            let (x, y) = (s.x + (i as i32 % 2) * half, line(1 + i as i32 / 2));
            cv.text(&f.s, x, y, l, DIM);
            cv.text(&f.sb, x + half - 8 - f.sb.width(v), y, v, TEXT);
        }
    }
    let Some(p) = peaks else { return };
    let y0 = cur_rows;
    let spec = match ui.m.spec_src {
        WaveId::Ch(c) => ui.ch_name(c),
        WaveId::Ref(i) => format!("R{}", i + 1),
    };
    cv.text(&f.sb, s.x, line(y0), &format!("Peaks  \u{b7}  {}  \u{b7}  dBV", spec), TEXT);
    if p.rows().next().is_none() {
        cv.text(&f.s, s.x, line(y0 + 1), "No lines above the noise", DIM);
    }
    let per = PEAKS_MAX.div_ceil(2);
    for (i, (number, m, held)) in p.rows().enumerate() {
        let color = if held { DIM } else { ACCENT };
        let (x, y) = (s.x + (i / per) as i32 * half, line(y0 + 1 + (i % per) as i32));
        let tag = Rect::new(x, y - 11, 15, 14);
        cv.round(tag, 3, color, 255);
        cv.text_in(&f.sb, tag, 1, &format!("{}", number), DARK_TEXT);
        cv.text(&f.s, x + 19, y, &si_digits(m.f, "Hz", 4), if held { DIM } else { LABEL });
        let v = format!("{:.1}", m.dbv);
        cv.text(&f.sb, x + half - 8 - f.sb.width(&v), y, &v, if held { DIM } else { TEXT });
    }
}
/// Geometry depends only on available space and configured row capacity.
struct PeakLayout {
    pitch: i32,
    facts: i32,
    first: i32,
    step: i32,
}

impl PeakLayout {
    fn new(r: Rect, rows: i32, facts: usize, pane: bool) -> Self {
        let nf = (facts as i32).min((r.h - rows * 18) / 16).max(0);
        let pitch = if rows > 0 { ((r.h - nf * 16) / rows).min(if nf < facts as i32 { r.h } else { 46 }) } else { 0 };
        let y0 = r.y + rows * pitch;
        let step = if nf > 0 { ((r.bottom() - y0) / nf).min(if pane { 40 } else { 24 }) } else { 0 };
        Self { pitch, facts: nf, first: y0 + (step + 9) / 2, step }
    }

    fn fact_y(&self, k: usize) -> i32 {
        self.first + k as i32 * self.step
    }
}

#[cfg(test)]
mod tests;
