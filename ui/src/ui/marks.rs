// SPDX-License-Identifier: GPL-2.0-or-later

//! Display > Dots = Auto, sparse samples (docs/fork/display.md): the trace is the interpolated line, and the samples
//! the scope really took are marked over it. The marks go on the layer over the traces, from the newest record only,
//! outside the phosphor maps: the grade of the pictures (`picture_ref`) stays that of the lines alone.

use super::*;

/// Mark positions in `area` for the samples `v` (sample j at `t0 + j * dt` s from the trigger) of a channel with
/// `offset` V and `vpd` V/div, shown at `view` (s/div, delay): the top-left corner of a 2 x 2 square centred on each
/// sample inside the area.
pub(super) fn sample_marks(area: Rect, view: (f64, f64), t0: f64, dt: f64, v: &[f32], offset: f32, vpd: f32) -> Vec<(i32, i32)> {
    let ppd_x = area.w as f64 / HDIV as f64;
    let ppd_y = area.h as f32 / VDIV;
    let (cx, cy) = (area.x as f64 + area.w as f64 / 2.0, area.y as f32 + area.h as f32 / 2.0);
    v.iter()
        .enumerate()
        .filter(|(_, y)| y.is_finite())
        .map(|(j, &y)| {
            let x = cx + (t0 + j as f64 * dt - view.1) / view.0 * ppd_x;
            let y = cy - (y + offset) / vpd * ppd_y;
            ((x - 0.5).floor() as i32, (y - 0.5).floor() as i32)
        })
        .filter(|&(x, y)| x >= area.x && y >= area.y && x + 1 < area.x + area.w && y + 1 < area.y + area.h)
        .collect()
}

impl Ui {
    /// Display > Dots from SCPI (`:DISPlay:TYPE`) or elsewhere: the maps start again in the new style.
    pub fn set_dots(&mut self, d: DotsMode) {
        self.m.dots = d;
        self.dirty_map = true;
    }

    /// Auto with samples at least `MARK_SPACING_PX` apart in `area` at `view` (s/div, delay): each sample of the newest
    /// record's frame marked over the line of its channel, lighter than the trace.
    pub(super) fn draw_sample_marks(&self, cv: &mut Canvas, area: Rect, view: (f32, f32)) {
        let Some(raw) = self.last[0].as_ref().and_then(|r| r.raw.as_ref()) else { return };
        let f = &raw.frame;
        if f.envelope || f.roll || f.scan {
            return;
        }
        let view = (view.0 as f64, view.1 as f64);
        let x_per_sample = f.dt / view.0 * area.w as f64 / HDIV as f64;
        if trace_style(self.m.dots, false, x_per_sample) != TraceStyle::VectorsMarked {
            return;
        }
        for c in 0..NCH {
            let ch = &self.m.ch[c];
            if !ch.on || !self.shows(c) || self.m.z_src == Some(c) {
                continue;
            }
            let Some(v) = f.ch[c].get(raw.i0..raw.i1) else { continue };
            let col = mix(CH_COLORS[c], rgb(255, 255, 255), 140);
            for (x, y) in sample_marks(area, view, raw.t0, f.dt, v, ch.offset, ch.volts_per_div()) {
                cv.fill(Rect::new(x, y, 2, 2), col);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_style_table() {
        use TraceStyle::*;
        let rows = [
            // mode, XY, px per sample, style
            (DotsMode::Off, false, 0.01, Vectors),
            (DotsMode::Off, false, 10.0, Vectors),
            (DotsMode::Off, true, 0.0, Vectors),
            (DotsMode::On, false, 0.01, Dots),
            (DotsMode::On, false, 10.0, Dots),
            (DotsMode::On, true, 0.0, Dots),
            (DotsMode::Auto, true, 0.0, Dots),
            (DotsMode::Auto, true, 10.0, Dots),
            (DotsMode::Auto, false, 0.01, Vectors),
            (DotsMode::Auto, false, 3.99, Vectors),
            (DotsMode::Auto, false, MARK_SPACING_PX, VectorsMarked),
            (DotsMode::Auto, false, 50.0, VectorsMarked),
        ];
        for (mode, xy, x, want) in rows {
            assert_eq!(trace_style(mode, xy, x), want, "{:?} xy {} {} px", mode, xy, x);
        }
        // the deposits draw dots only with On
        let mut m = Model::default();
        for (mode, dots) in [(DotsMode::Off, false), (DotsMode::On, true), (DotsMode::Auto, false)] {
            m.dots = mode;
            assert!([0.01, 1.0, 10.0].iter().all(|&x| m.dots_at(x) == dots), "{:?}", mode);
        }
    }

    // Break it catches: the query in another spelling, the long and short forms not taken, a bad value taken silently
    #[test]
    fn display_type_by_scpi() {
        let mut u = Ui::new(crate::wave::Source::Demo { t: 0.0 });
        let cv = Canvas::new(PANEL_W, PANEL_H);
        let mut scpi = crate::scpi::Scpi::open(0);
        assert_eq!(scpi.exec_line(&mut u, &cv, ":DISPlay:TYPE?"), b"AUTO");
        for (cmd, mode, answer) in [("VECTors", DotsMode::Off, &b"VECT"[..]), ("DOTS", DotsMode::On, b"DOTS"), ("auto", DotsMode::Auto, b"AUTO"), ("VECT", DotsMode::Off, b"VECT")] {
            u.dirty_map = false;
            scpi.exec_line(&mut u, &cv, &format!(":DISP:TYPE {}", cmd));
            assert_eq!((u.m.dots, u.dirty_map), (mode, true), "{}", cmd);
            assert_eq!(scpi.exec_line(&mut u, &cv, ":DISP:TYPE?"), answer);
        }
        scpi.exec_line(&mut u, &cv, ":DISP:TYPE LINES");
        assert_eq!(u.m.dots, DotsMode::Off);
        assert!(String::from_utf8_lossy(&scpi.exec_line(&mut u, &cv, ":SYST:ERR?")).starts_with("-224"));
    }

    // Break it catches: marks off the sample times (a pixel offset, the delay or the time base ignored), samples
    // outside the area marked
    #[test]
    fn marks_sit_on_the_sample_positions() {
        let area = Rect::new(100, 50, 1000, 400);
        // 100 ns/div, 10 div: 1 us across 1000 px; samples 10 ns apart = 10 px, the first at -500 ns (the left edge)
        let (tdiv, dt) = (100e-9, 10e-9);
        let v: Vec<f32> = (0..101).map(|j| if j % 2 == 0 { 0.0 } else { 1.0 }).collect();
        let marks = sample_marks(area, (tdiv, 0.0), -500e-9, dt, &v, 0.0, 1.0);
        // the left-most sample sits on the area's edge: its square would leave the area
        assert_eq!(marks.len(), 99);
        for &(x, y) in &marks {
            // the square's centre is the sample: x + 1 = 100 + 500 + t / 1 ns, y + 1 = 250 (0 V) or 200 (1 V at 1 V/div)
            let j = ((x + 1 - 100) as f64 / 10.0).round() as usize;
            assert_eq!(x + 1, 100 + (j * 10) as i32, "x of sample {}", j);
            assert_eq!(y + 1, if j % 2 == 0 { 250 } else { 200 }, "y of sample {}", j);
        }
        // the delay moves them along: 20 ns = 20 px to the left
        let moved = sample_marks(area, (tdiv, 20e-9), -500e-9, dt, &v, 0.0, 1.0);
        // samples 3 to 100 in the area now: the last one was on the right edge before
        assert_eq!(moved.len(), 98);
        assert!(moved.iter().all(|&(x, y)| x + 1 + 20 == 1100 || marks.contains(&(x + 20, y))));
    }

    // Break it catches: marks in On or Off, marks where the samples are close (the line is all samples there), marks
    // from pair frames
    #[test]
    fn only_auto_marks_and_only_sparse_samples() {
        let mut u = Ui::new(crate::wave::Source::Demo { t: 0.0 });
        let area = Rect::new(0, 0, 1000, 400);
        let n = 101usize;
        let frame = |dt: f64, envelope: bool| crate::board::Frame { seq: 1, dt, pre: n / 2, ch: [vec![0.1; n], Vec::new(), Vec::new(), Vec::new()], envelope, ..Default::default() };
        let rec = |f: crate::board::Frame| {
            let f = std::sync::Arc::new(f);
            let t0 = -(f.pre as f64) * f.dt;
            Record { t0, dt: f.dt, ch: Default::default(), triggered: true, partial: false, raw: Some(Raw { frame: f, i0: 0, i1: n, t0 }), la: None, pairs: false }
        };
        for c in 1..NCH {
            u.m.ch[c].on = false;
        }
        let tdiv = 100e-9f32;
        let lit = |u: &Ui| {
            let mut cv = Canvas::new(1000, 400);
            u.draw_sample_marks(&mut cv, area, (tdiv, 0.0));
            cv.px.iter().filter(|&&p| p != 0).count()
        };
        // 10 px apart
        u.last[0] = Some(rec(frame(10e-9, false)));
        for (mode, want) in [(DotsMode::Auto, true), (DotsMode::On, false), (DotsMode::Off, false)] {
            u.m.dots = mode;
            assert_eq!(lit(&u) > 0, want, "{:?}", mode);
        }
        u.m.dots = DotsMode::Auto;
        // 2 px apart: below MARK_SPACING_PX
        u.last[0] = Some(rec(frame(2e-9, false)));
        assert_eq!(lit(&u), 0);
        // min/max pairs are no samples
        u.last[0] = Some(rec(frame(10e-9, true)));
        assert_eq!(lit(&u), 0);
    }
}
