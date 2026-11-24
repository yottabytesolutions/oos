// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::fastdraw::edge_smooth;
use oos_dsp::sim;

#[test]
fn integer_smoothing_changes_fractional_edge_bits() {
    let cal = crate::fastdraw::CorrectedCodes { offsets: [0; 8], scale: 1.25, offset: 0.375 };
    let codes = [24001u16, 24001, 35021, 41001, 41001, 41001];
    let volts = codes.map(|code| cal.volts(code));
    let (legacy, _) = edge_smooth(
        volts.as_slice(),
        3,
        EdgeSearch { level: 0.0, hysteresis: 0.1, rising: true, either: false },
        3,
        2,
        |_| true,
    );
    let threshold = 32768.0 + 7500.0 * cal.offset / cal.scale;
    let sum_before = u32::from(codes[1]) + u32::from(codes[2]);
    let sum_after = u32::from(codes[2]) + u32::from(codes[3]);
    let integer = 2.0 + (2.0 * threshold - f64::from(sum_before)) / f64::from(sum_after - sum_before);
    let legacy = legacy.unwrap();
    assert!((legacy - integer).abs() < 1e-6);
    assert_ne!(legacy.to_bits(), integer.to_bits());
}

/// Frames of the guard corpus: name, samples.
pub(in crate::trignear) fn corpus() -> Vec<(String, Vec<f32>)> {
    let mut out = Vec::new();
    let n = 40_000;
    // fast square, slow square (edges over hundreds of samples), sine, each clean and noisy
    for (name, f, tr) in [("square", 1e5, 2e-8), ("slow-edge", 2e4, 4e-6), ("square-hf", 2e6, 1e-9)] {
        let p = sim::PulseTrain::square(f, -1.0, 1.0, tr);
        for noise in [0.0f32, 0.05, 0.4] {
            let mut v = sim::sample(|t| p.v(t), 3.3e-7, 1e-9, n);
            sim::add_noise(&mut v, noise, 7);
            out.push((format!("{} noise {}", name, noise), v));
        }
    }
    for noise in [0.0f32, 0.02, 0.3] {
        let mut v = sim::sample(|t| (t * 3.1e4 * std::f64::consts::TAU).sin(), 0.0, 1e-9, n);
        sim::add_noise(&mut v, noise, 11);
        out.push((format!("sine noise {}", noise), v));
    }
    // a not so clean logic signal (scaled to -1..1): ringing after each edge and noise of +-0.4
    let mut ring: Vec<f32> = (0..n)
        .map(|k| {
            let ph = k % 2000;
            let base: f32 = if ph < 1000 { -1.0 } else { 1.0 };
            let since = (ph % 1000) as f32;
            base - base.signum() * 0.5 * (-since / 40.0).exp() * (since / 6.0).cos()
        })
        .collect();
    sim::add_noise(&mut ring, 0.4, 13);
    out.push(("ringing logic noise 0.4".into(), ring));
    // runts: pulses to half height between full pulses
    let runt: Vec<f32> = (0..n)
        .map(|k| match (k / 500) % 4 {
            0 => 1.0,
            2 => 0.1,
            _ => -1.0,
        })
        .collect();
    out.push(("runts".into(), runt));
    let mut rp = sim::PulseTrain::square(1e5, -1.0, 1.0, 5e-8);
    (rp.every, rp.special) = (3, sim::Pulse { width: 5e-6, height: 0.45 });
    let mut v = sim::sample(|t| rp.v(t), 1.1e-6, 1e-9, n);
    sim::add_noise(&mut v, 0.03, 5);
    out.push(("runt-train".into(), v));
    // quantised like decoded ADC words: many samples exactly on the level
    let q: Vec<f32> = (0..n).map(|k| ((k as f32 / 900.0).sin() * 40.0).round() / 40.0).collect();
    out.push(("quantised".into(), q));
    // flat with a single glitch, and constant: no edge at all
    let mut g = vec![0.0f32; n];
    g[n / 2 + 37] = 1.0;
    out.push(("glitch".into(), g));
    out.push(("flat".into(), vec![0.25f32; n]));
    out
}

fn same(a: (Option<f64>, Option<Edge>), b: (Option<f64>, Option<Edge>)) -> bool {
    let t = |x: Option<f64>, y: Option<f64>| match (x, y) {
        (None, None) => true,
        (Some(x), Some(y)) => (x - y).abs() < 1e-6,
        _ => false,
    };
    t(a.0, b.0) && t(a.1.map(|e| e.t), b.1.map(|e| e.t)) && a.1.map(|e| e.rising) == b.1.map(|e| e.rising)
}

// Break it catches: a near search that returns an edge the wide search would not (a closer edge it could not see,
// an edge the comparator only decides outside the near window, a different edge of either direction).
#[test]
fn near_first_equals_the_wide_search() {
    let (mut cases, mut hits) = (0, 0);
    for (name, v) in corpus() {
        // anywhere, and where the hardware puts it: on or next to an edge
        let at_edges = oos_dsp::edges::trigger_edges(&v, 0.0, 0.1)
            .into_iter()
            .map(|e| e.t as usize)
            .filter(|&t| t > 300 && t < v.len() - 300)
            .step_by(7)
            .take(12);
        let pres: Vec<usize> =
            (300..v.len() - 300).step_by(4999).chain(at_edges.flat_map(|t| [t, t + 3, t - 40])).collect();
        for pre in pres {
            for (l, span) in [(1, 600), (4, 2000), (16, 9000), (62, 19_000)] {
                for hyst in [0.0f32, 0.06, 0.3] {
                    // -0.97: next to the low rail, where noise moves around the level but never arms a rising edge
                    for level in [0.0f32, 0.3, -0.5, -0.97] {
                        for (rising, either) in [(true, false), (false, false), (true, true)] {
                            let trigger = crate::fastdraw::EdgeSearch { level, hysteresis: hyst, rising, either };
                            let fits_all = |_: f64| true;
                            let lo = pre as f64 - 50.0;
                            let fits_some = |t: f64| t >= lo;
                            let w = edge_smooth(v.as_slice(), pre, trigger, span, l, fits_all);
                            let n = edge_smooth_near(&v, pre, level, hyst, rising, either, span, l, fits_all);
                            assert!(
                                same(w, n),
                                "{} pre {} l {} hyst {} level {} {:?}/{}: wide {:?} near {:?}",
                                name,
                                pre,
                                l,
                                hyst,
                                level,
                                rising,
                                either,
                                w,
                                n
                            );
                            let w = edge_smooth(v.as_slice(), pre, trigger, span, l, fits_some);
                            let n = edge_smooth_near(&v, pre, level, hyst, rising, either, span, l, fits_some);
                            assert!(same(w, n), "{} (fits) pre {} l {}: wide {:?} near {:?}", name, pre, l, w, n);
                            cases += 2;
                            hits += near_only(&v, (0, v.len()), (pre, NEAR_COLS * l + NEAR_PAD, l), trigger, &fits_all)
                                .is_some() as usize;
                        }
                    }
                }
            }
        }
    }
    // the near search must answer a fair share by itself, or it only costs time
    println!("{} cases, the near search answered {}", cases / 2, hits);
    assert!(cases > 10_000 && hits * 5 > cases / 2, "{} cases, near answered {}", cases / 2, hits);
}

// Break it catches: a near result accepted although a closer wide edge depends on what lies before the near
// window: the comparator was armed (or not) by a sample the near window does not see
#[test]
fn near_first_falls_back_when_the_window_cannot_decide() {
    let pre = 1000;
    // +1 long before, then just above the level (inside the band) into the near window, a falling crossing at
    // pre - 20 that only the history armed, -1 from pre + 5, a rising edge far away at pre + 200
    let armed_before: Vec<f32> = (0..6000)
        .map(|k| {
            if k < pre - 300 {
                1.0
            } else if k < pre - 20 {
                0.05
            } else if k < pre + 5 {
                -0.05
            } else if k < pre + 200 {
                -1.0
            } else {
                1.0
            }
        })
        .collect();
    // -1, then just below the level from pre - 200, a rising crossing at pre - 75 inside the band, +1 from pre - 60,
    // falling at pre + 79
    let early: Vec<f32> = (0..6000)
        .map(|k| {
            if k < pre - 200 {
                -1.0
            } else if k < pre - 75 {
                -0.05
            } else if k < pre - 60 {
                0.05
            } else if k < pre + 79 {
                1.0
            } else {
                -1.0
            }
        })
        .collect();
    for (v, rising) in [(armed_before, false), (early, true)] {
        let wide = edge_smooth(
            v.as_slice(),
            pre,
            crate::fastdraw::EdgeSearch { level: 0.0, hysteresis: 0.3, rising: true, either: true },
            4000,
            1,
            |_| true,
        );
        assert!(wide.1.is_some_and(|e| e.rising == rising), "{:?}", wide);
        let trigger = EdgeSearch { level: 0.0, hysteresis: 0.3, rising: true, either: true };
        assert!(near_only(&v, (0, v.len()), (pre, NEAR_COLS + NEAR_PAD, 1), trigger, &|_| true).is_none());
        assert!(same(wide, edge_smooth_near(&v, pre, 0.0, 0.3, true, true, 4000, 1, |_| true)));
    }
}

// Break it catches (`edge_near`, AC and LF-reject trigger coupling): an edge outside the near window (a crossing
// the hardware did not trigger on), another edge than the nearest within it, or another answer from the window the
// codes path decodes than from the whole frame.
#[test]
fn filtered_path_answers_from_the_near_window_only() {
    let (mut cases, mut found) = (0, 0);
    for (name, v) in corpus() {
        for pre in (300..v.len() - 300).step_by(1733) {
            for (l, span) in [(1, 600), (4, 2000), (16, 9000), (62, 19_000)] {
                let near = (NEAR_COLS * l + NEAR_PAD).min(span);
                let (a, b) = near_window(pre, v.len(), l);
                for hyst in [0.0f32, 0.06, 0.3] {
                    for level in [0.0f32, 0.3, -0.97] {
                        for (rising, either) in [(true, false), (false, false), (true, true)] {
                            let r = edge_near(&v, 0, v.len(), pre, level, hyst, rising, either, span, l);
                            let what = format!(
                                "{} pre {} l {} hyst {} level {} {:?}/{}",
                                name, pre, l, hyst, level, rising, either
                            );
                            assert!(
                                same(
                                    r,
                                    edge_smooth(
                                        v.as_slice(),
                                        pre,
                                        crate::fastdraw::EdgeSearch { level, hysteresis: hyst, rising, either },
                                        near,
                                        l,
                                        |_| true
                                    )
                                ),
                                "{}: {:?}",
                                what,
                                r
                            );
                            assert!(
                                same(r, edge_near(&v[a..b], a, v.len(), pre, level, hyst, rising, either, span, l)),
                                "{}: window",
                                what
                            );
                            if let Some(t) = r.0 {
                                assert!(
                                    (t - pre as f64).abs() <= (near + l) as f64,
                                    "{}: {} from pre",
                                    what,
                                    t - pre as f64
                                );
                                found += 1;
                            }
                            cases += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(cases > 5000 && found * 10 > cases, "{} cases, {} edges", cases, found);
    // the case it is for: one edge, far from the hardware point; the wide search finds it, the filtered path not
    let far: Vec<f32> = (0..40_000).map(|k| if k < 30_000 { -1.0 } else { 1.0 }).collect();
    assert!(edge_smooth_near(&far, 20_000, 0.0, 0.1, true, false, 19_000, 16, |_| true)
        .0
        .is_some_and(|t| (t - 30_000.0).abs() < 1.0));
    assert!(same(edge_near(&far, 0, far.len(), 20_000, 0.0, 0.1, true, false, 19_000, 16), (None, None)));
}
