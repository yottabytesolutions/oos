// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn a_limit_catches_what_rises_above_it() {
    let mut sp = Spectrum { f0: 0.0, df: 10.0, dbv: vec![-60.0; 100], rbw: 10.0, ..Default::default() };
    sp.dbv[50] = -20.0;
    let l = limit_from(&sp, 6.0, 2);
    assert_eq!((l.len(), l[48].1, l[47].1), (100, -14.0, -54.0));
    assert_eq!(limit_check(&sp, &l), None);
    assert_eq!(limit_at(&l, 5.0), Some(-54.0));
    assert_eq!(limit_at(&l, 1e6), None);
    sp.dbv[10] = -50.0;
    sp.dbv[80] = -40.0;
    let (f, over) = limit_check(&sp, &l).unwrap();
    assert_eq!((f, over), (800.0, 14.0));
}

fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|x| x.to_bits()).collect()
}

// Compare all three retained traces with the indexed update, including the averaging limit.
fn compare_trace_updates(settings: Settings) {
    let mut state: Option<Avg> = None;
    for frame in 0..7 {
        let wave: Vec<f32> = (0..1024)
            .map(|i| {
                let phase = 2.0 * PI * i as f64 / 1024.0;
                ((1.0 + frame as f64 * 0.1) * (17.25 * phase + frame as f64).sin()
                    + 0.2 * (79.5 * phase).cos()
                    + 0.05 * frame as f64) as f32
            })
            .collect();
        let mut fresh = None;
        analyze(&wave, 1e-6, &settings, &mut fresh);
        let current = fresh.expect("a finite record creates a trace").max;
        let mut expected = state.clone().unwrap_or_else(|| Avg {
            avg: current.clone(),
            max: current.clone(),
            min: current.clone(),
            ..Default::default()
        });
        expected.n = expected.n.saturating_add(1);
        let weight = 1.0 / expected.n.min(settings.averaging.max(1)) as f64;
        let mut i = 0;
        while i < current.len() {
            expected.avg[i] += (current[i] - expected.avg[i]) * weight;
            expected.max[i] = expected.max[i].max(current[i]);
            expected.min[i] = expected.min[i].min(current[i]);
            i += 1;
        }
        let output = analyze(&wave, 1e-6, &settings, &mut state);
        let actual = state.as_ref().expect("a finite record creates a trace");
        assert_eq!(actual.n, expected.n);
        assert_eq!(bits(&actual.avg), bits(&expected.avg));
        assert_eq!(bits(&actual.max), bits(&expected.max));
        assert_eq!(bits(&actual.min), bits(&expected.min));
        let selected = match settings.trace {
            Trace::Normal => &current,
            Trace::Average => &expected.avg,
            Trace::MaxHold => &expected.max,
            Trace::MinHold => &expected.min,
        };
        let expected_dbv: Vec<u32> =
            selected.iter().map(|p| (10.0 * p.max(1e-30).log10()) as f32).map(f32::to_bits).collect();
        assert_eq!(output.dbv.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), expected_dbv);
    }
}

#[test]
fn trace_updates_match_indexed_calculation_bit_for_bit() {
    for window in [Window::Rect, Window::Hann, Window::Hamming, Window::Blackman, Window::Flattop] {
        for detector in [Detector::Peak, Detector::Sample, Detector::Avg] {
            for trace in [Trace::Normal, Trace::Average, Trace::MaxHold, Trace::MinHold] {
                for points in [2, 31, 97] {
                    compare_trace_updates(Settings {
                        window,
                        detector,
                        trace,
                        points,
                        averaging: 3,
                        ..Default::default()
                    });
                }
            }
        }
    }
}
