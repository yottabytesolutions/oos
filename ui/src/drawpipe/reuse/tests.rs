// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{sparse::Interp, wave::Phosphor};
use std::sync::Arc;

fn fixture() -> (Model, Arc<Frame>) {
    let mut m =
        Model { tscale: tscales().iter().position(|&t| (t - 1e-3).abs() < 1e-9).unwrap(), ..Default::default() };
    for c in &mut m.ch {
        c.on = true;
    }
    let f = Frame {
        seq: 1,
        dt: 51.2e-9,
        pre: 312_500,
        envelope: true,
        acq: (312.5e6, 10_000_000),
        ch: std::array::from_fn(|c| (0..625_000).map(|i| ((i / 2 + c) as f32 * 0.003).sin()).collect()),
        ..Default::default()
    };
    (m, Arc::new(f))
}

#[test]
fn reuse_is_pixel_exact_and_skips_the_second_resample() {
    let (mut m, f) = fixture();
    for (delay, slope, level) in [(0.0, Slope::Rise, 0.0), (0.002, Slope::Fall, 0.3), (-0.03, Slope::Either, 8.0)] {
        (m.delay, m.slope, m.trig_level) = (delay, slope, level);
        let input = EdgeInputs::of(&m).unwrap();
        let rec = crate::wave::hw_record(&m, f.clone());
        assert!(input.matches(&m, &f, &rec));
        let (mut old, mut new) = (Phosphor::new(970, 448), Phosphor::new(970, 448));
        let before = crate::board::record::resample_count();
        let expected = crate::drawpipe::draw_frame(&mut old, &m, &f, Interp::Exact, None);
        assert_eq!(crate::board::record::resample_count() - before, 1);
        let before = crate::board::record::resample_count();
        let actual = crate::drawpipe::draw_frame_reusing(&mut new, &m, &f, Interp::Exact, None, Some((&rec, input)));
        assert_eq!(crate::board::record::resample_count() - before, 0);
        assert_eq!(actual, expected);
        assert_eq!(new.maps(), old.maps());
    }
}

#[test]
fn reuse_rejects_different_conversion_inputs_and_frame_identity() {
    let (m, f) = fixture();
    let input = EdgeInputs::of(&m).unwrap();
    let rec = crate::wave::hw_record(&m, f.clone());
    let edits: [fn(&mut Model); 11] = [
        |m| m.delay += 0.001,
        |m| m.tscale += 1,
        |m| m.trig_src = 1,
        |m| m.slope = Slope::Fall,
        |m| m.trig_level += 0.1,
        |m| m.trig_coup = TrigCoup::Ac,
        |m| m.ch[0].vscale += 1,
        |m| m.ch[0].probe *= 10.0,
        |m| m.trig_nrej = !m.trig_nrej,
        |m| m.acq = AcqMode::HiRes,
        |m| m.trig_type = TrigType::Pulse,
    ];
    for edit in edits {
        let mut changed = m.clone();
        edit(&mut changed);
        assert!(!input.matches(&changed, &f, &rec));
        let (mut old, mut new) = (Phosphor::new(970, 448), Phosphor::new(970, 448));
        let expected = crate::drawpipe::draw_frame(&mut old, &changed, &f, Interp::Exact, None);
        let before = crate::board::record::resample_count();
        let got = crate::drawpipe::draw_frame_reusing(&mut new, &changed, &f, Interp::Exact, None, Some((&rec, input)));
        assert_eq!(crate::board::record::resample_count() - before, 1);
        assert_eq!(got, expected);
        assert_eq!(new.maps(), old.maps());
    }
    assert!(!input.matches(&m, &(*f).clone(), &rec));
    let mut wrong_grid = rec.clone();
    wrong_grid.dt *= 2.0;
    assert!(!input.matches(&m, &f, &wrong_grid));
    wrong_grid = rec.clone();
    wrong_grid.ch[0].pop();
    assert!(!input.matches(&m, &f, &wrong_grid));
}

#[test]
#[ignore = "release ratio guard, run by tools/perf-guard.sh"]
fn perf_guard_deep_record_reuse() {
    let (m, frame) = fixture();
    let (mut old, mut new) = (Phosphor::new(970, 448), Phosphor::new(970, 448));
    let convert_and_draw = |ph: &mut Phosphor, reuse| {
        ph.clear();
        let input = EdgeInputs::of(&m).unwrap();
        // Actual Source::acquire conversion, followed by the actual draw_frame fallback.
        // The reference independently triggers/resamples again; no synthetic copy/work.
        let rec = crate::wave::hw_record(&m, frame.clone());
        std::hint::black_box(&rec);
        let triggered = if reuse {
            crate::drawpipe::draw_frame_reusing(ph, &m, &frame, Interp::Exact, None, Some((&rec, input)))
        } else {
            crate::drawpipe::draw_frame(ph, &m, &frame, Interp::Exact, None)
        };
        std::hint::black_box(&ph.maps());
        triggered
    };
    let before = crate::board::record::resample_count();
    convert_and_draw(&mut old, false);
    assert_eq!(crate::board::record::resample_count() - before, 2);
    let before = crate::board::record::resample_count();
    convert_and_draw(&mut new, true);
    assert_eq!(crate::board::record::resample_count() - before, 1);
    assert_eq!(new.maps(), old.maps());
    let pair = crate::system::perfguard::measure(
        crate::perfguard::ROUNDS,
        3,
        || {
            std::hint::black_box(convert_and_draw(&mut old, false));
        },
        || {
            std::hint::black_box(convert_and_draw(&mut new, true));
        },
    );
    pair.report("perf_guard_deep_record_reuse");
    println!("10M depth, 4 x 625k pairs, real double conversion/reuse and draw: {pair:?}");
    assert!(pair.best.1.as_secs_f64() / pair.best.0.as_secs_f64() <= 0.8);
    pair.check_tail(1.0);
}
