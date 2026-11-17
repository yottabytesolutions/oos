// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

#[test]
fn fluctuating_peak_counts_reserve_rows_and_keep_all_summary_positions() {
    let mut u = Ui::for_tests(Source::Demo { t: 0.0 });
    u.step(0.04);
    u.now = 0.0;
    u.azr.spec = Some(spectrum::Spectrum { df: 1.0, rbw: 4.0, ..Default::default() });
    for pane in [false, true] {
        for area in [Rect::new(0, 40, 1024, 510), Rect::new(0, 40, 800, 360), Rect::new(0, 350, 1024, 230)] {
            let b = u.spec_box(area, pane);
            let expected = PeakLayout::new(b.list, PEAKS_MAX.min((b.list.h / 18) as usize) as i32, 14, pane);
            for count in [3, 4, 3, 4, 0, 8] {
                u.azr.work.peak_table = Default::default();
                u.azr.peaks = (0..count).map(|i| spectrum::Marker { f: (i + 1) as f64 * 1e3, dbv: -10.0 }).collect();
                u.now += 1.0;
                u.track_peaks();
                u.now += 1.0;
                u.track_peaks();
                assert_eq!(u.azr.work.peak_table.rows().count(), count);
                let rows = u.peaks_shown(&b) as i32;
                assert_eq!(rows, PEAKS_MAX.min((b.list.h / 18) as usize) as i32);
                let layout = PeakLayout::new(b.list, rows, 14, pane);
                for k in 0..14 {
                    assert_eq!(layout.fact_y(k), expected.fact_y(k), "summary row {k}, peaks {count}");
                }
            }
        }
    }
}

fn pixels(cv: &Canvas, r: Rect) -> Vec<u32> {
    (r.y..r.bottom()).flat_map(|y| cv.px[(y * cv.w + r.x) as usize..(y * cv.w + r.right()) as usize].to_vec()).collect()
}

#[test]
fn empty_peak_placeholders_cover_qualification_and_release_in_list_and_results() {
    let mut u = Ui::for_tests(Source::Demo { t: 0.0 });
    let sp = spectrum::Spectrum { df: 1.0, rbw: 4.0, ..Default::default() };
    u.azr.spec = Some(sp.clone());
    let raw = spectrum::Marker { f: 100.0, dbv: -10.0 };
    for (now, present, empty) in [(0.0, true, true), (1.0, true, false), (1.1, false, false), (2.1, false, true)] {
        u.now = now;
        u.azr.peaks = if present { vec![raw] } else { vec![] };
        u.track_peaks();
        assert_eq!(u.azr.work.peak_table.rows().next().is_none(), empty);
        for view in 0..3 {
            let mut actual = Canvas::new(u.scr.w, u.scr.h);
            let (x, y) = if view < 2 {
                let b = u.spec_box(Rect::new(0, 40, 1024, 510), view == 1);
                u.draw_peaks(&mut actual, &b, &sp, view == 1);
                (b.list.x + 4, b.list.y + 14)
            } else {
                let side = Rect::new(10, 10, 400, 200);
                u.draw_results_side(&mut actual, side);
                (side.x, side.y + 40 + (40 + 9) / 2)
            };
            let mut expected = Canvas::new(u.scr.w, u.scr.h);
            let font = &u.fonts().s;
            let message = "No lines above the noise";
            expected.text(font, x, y, message, DIM);
            let r = Rect::new(x, y - 13, font.width(message), 15);
            let text = pixels(&expected, r);
            assert!(text.iter().any(|&p| p != 0), "placeholder must have visible glyphs");
            assert_eq!(pixels(&actual, r) == text, empty, "view {view} at {now}s");
        }
    }
}

#[test]
fn summary_pixels_stay_fixed_while_a_fourth_peak_qualifies_and_releases() {
    let mut u = Ui::for_tests(Source::Demo { t: 0.0 });
    u.step(0.04);
    u.azr.spec = Some(spectrum::Spectrum { df: 1.0, rbw: 4.0, floor_dbv: -80.0, ..Default::default() });
    let raw: [_; 4] = std::array::from_fn(|i| spectrum::Marker { f: 100.0 * (i + 1) as f64, dbv: -10.0 * i as f64 });
    let b = u.spec_box(u.area, false);
    let sp = u.azr.spec.as_ref().unwrap().clone();
    let layout = PeakLayout::new(b.list, u.peaks_shown(&b) as i32, spec_facts(&u, &sp, false).len(), false);
    let y0 = (layout.first - (layout.step + 9) / 2) as usize;
    let mut saved = None;
    for (now, count) in [(0.0, 3), (1.0, 3), (1.1, 4), (2.1, 4), (2.2, 3), (3.19, 3), (3.2, 3)] {
        u.now = now;
        u.azr.peaks = raw[..count].to_vec();
        u.track_peaks();
        let mut cv = Canvas::new(u.scr.w, u.scr.h);
        u.draw_peaks(&mut cv, &b, &sp, false);
        let pixels = (y0..b.list.bottom() as usize)
            .flat_map(|y| {
                cv.px[y * cv.w as usize + b.list.x as usize..y * cv.w as usize + b.list.right() as usize].to_vec()
            })
            .collect::<Vec<_>>();
        if let Some(ref saved) = saved {
            assert_eq!(&pixels, saved, "summary moved at {now}s with {count} raw peaks");
        } else {
            saved = Some(pixels);
        }
    }
}

#[test]
fn unavailable_distortion_and_limit_values_keep_summary_row_labels() {
    let mut u = Ui::for_tests(Source::Demo { t: 0.0 });
    u.m.spec_dist = true;
    let sp = spectrum::Spectrum::default();
    let labels = |u: &Ui| spec_facts(u, &sp, false).into_iter().map(|v| v.0).collect::<Vec<_>>();
    let missing = labels(&u);
    assert_eq!(&missing[..7], ["f1", "THD", "THD+N", "SNR", "SINAD", "SFDR", "ENOB"]);
    let v: Vec<_> = (0..4096).map(|i| (std::f64::consts::TAU * i as f64 / 64.0).sin() as f32).collect();
    u.azr.dist = spectrum::distortion(&v, 1e-6, None, 10);
    assert!(u.azr.dist.is_some());
    assert_eq!(labels(&u), missing);
    u.azr.limit = Some(spectrum::limit_from(&sp, 3.0, 8));
    let before = labels(&u);
    u.azr.limit_worst = Some((1e3, 3.0));
    assert_eq!(labels(&u), before);
}

#[test]
fn full_view_keeps_all_fourteen_summary_rows_visible_with_eight_peak_slots() {
    let mut u = Ui::for_tests(Source::Demo { t: 0.0 });
    u.step(0.04);
    u.m.view = AView::Spectrum;
    u.m.spec_dist = true;
    u.relayout();
    let b = u.spec_box(u.area, false);
    assert!(!overlaps(b.sort, u.view_btn()));
    assert!(!overlaps(b.sort, u.full_home().1));
    let facts = spec_facts(&u, &spectrum::Spectrum::default(), false);
    let layout = PeakLayout::new(b.list, u.peaks_shown(&b) as i32, facts.len(), false);
    assert_eq!(u.peaks_shown(&b), 8);
    assert_eq!(layout.facts, 14);
    assert!(layout.pitch >= 18);
    assert!(layout.fact_y(13) <= b.list.bottom());
    assert_eq!(facts[13].0, "Trace");
}
