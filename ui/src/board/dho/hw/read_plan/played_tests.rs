// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::{
    decode::{codes_into, decode},
    header::Fixture,
    window::{batch_play, configured_play_window, final_replay, raw_replay_settings},
};
use crate::{
    board::Frame,
    drawpipe::{self, metrics::fallback, test_scope::Scope},
    fastdraw::{Samples, Spans},
    model::Model,
    picture::deposit::{kernel, standard},
};
use oos_hal::dho::{hz, pairs, seq, seq::Settings};
use std::path::Path;

mod cropped;

struct ViewCase {
    name: &'static str,
    geometry: (u32, u32, u32),
    bytes: (usize, usize),
}

const VIEWS: [ViewCase; 5] = [
    ViewCase { name: "1ch-1us", geometry: (360, 13344, 3), bytes: (9392, 8912) },
    ViewCase { name: "2ch-1us", geometry: (0, 7032, 1), bytes: (28144, 28144) },
    ViewCase { name: "1ch-10us", geometry: (16560, 66840, 15), bytes: (13344, 8928) },
    ViewCase { name: "4ch-10us", geometry: (952, 33264, 7), bytes: (40144, 38032) },
    ViewCase { name: "1ch-10us-1M", geometry: (434000, 131936, 31), bytes: (64528, 8528) },
];

#[test]
fn recorded_columns_match_whole_record_play() {
    let _scope = Scope::new();
    let external = std::env::var_os("OOS_PLAY_WINDOW_FIXTURES").map(std::path::PathBuf::from);
    let embedded = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/played-window");
    let mut differing_views = Vec::new();
    for ViewCase { name, geometry, bytes } in VIEWS {
        let paths: Vec<_> = if let Some(root) = &external {
            let mut paths: Vec<_> = std::fs::read_dir(root.join(name))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.join("fixture.txt").is_file())
                .collect();
            paths.sort();
            assert!(!paths.is_empty(), "{name}");
            paths
        } else {
            vec![embedded.join(name)]
        };
        let mut records = 0;
        let mut differences = [0; 3];
        for path in paths {
            for (total, count) in differences.iter_mut().zip(compare_fixture(&path, geometry, bytes, &mut records)) {
                *total += count;
            }
        }
        println!("{name}: {records} records, (lo, n, k)={geometry:?}, differing lane-columns (envelope, batch, fast)={differences:?}");
        if differences != [0; 3] {
            differing_views.push((name, differences));
        }
    }
    assert!(differing_views.is_empty(), "{differing_views:?}");
}

fn compare_fixture(path: &Path, geometry: (u32, u32, u32), bytes: (usize, usize), records: &mut usize) -> [usize; 3] {
    let fixture = Fixture::load(path).unwrap();
    let (raw, p) = fixture.settings();
    let (m, size, _, _) = fixture.view().unwrap().inputs();
    assert!(crate::compress::allowed(m));
    let k = crate::compress::k_for(p.rate, raw.tdiv, p.memsel);
    let whole = Settings { compress: k, ..raw.clone() };
    let crop = batch_play(&whole, p, 1, configured_play_window(&whole, p, size.0));
    let (n, lo) = seq::window(&crop, p);
    assert_eq!((lo, n, k), geometry, "{path:?}");
    assert_eq!((seq::frame_bytes(&whole, p), seq::frame_bytes(&crop, p)), bytes, "{path:?}");
    let mut totals = [0; 3];
    for raw in fixture.records() {
        let (a, b) = (fpga_play(raw, &whole, p), fpga_play(raw, &crop, p));
        let ((envelope, batch), fast) = (
            compare_columns((m, size), p, (&whole, &a), (&crop, &b)),
            compare_fast((m, size), p, (&whole, &a), (&crop, &b)),
        );
        for (total, count) in totals.iter_mut().zip([envelope, batch, fast]) {
            *total += count;
        }
        *records += 1;
    }
    totals
}

fn fpga_play(raw: &[u8], s: &Settings, p: &hz::Params) -> Vec<u8> {
    let (n, lo) = seq::window(s, p);
    let k = seq::compress_k(s, p) as usize;
    if k == 1 {
        return raw.to_vec();
    }
    let len = seq::compressed_len(s, p) as usize;
    let lanes = p.ratio as usize;
    let words_per_point = 2 * lanes;
    assert!(len * k <= n as usize);
    let start = 16 + lo as usize * words_per_point;
    let payload = &raw[start..start + len * k * words_per_point];
    let mut played = vec![0; seq::frame_bytes(s, p)];
    played[..16].copy_from_slice(&raw[..16]);
    for lane in 0..lanes {
        let codes: Vec<u16> = payload
            .chunks_exact(words_per_point)
            .map(|point| u16::from_le_bytes([point[2 * lane], point[2 * lane + 1]]))
            .collect();
        let compressed = pairs::model(&codes, k, pairs::LAYOUT);
        assert_eq!(compressed.len(), len);
        for (point, code) in played[16..].chunks_exact_mut(words_per_point).zip(compressed) {
            point[2 * lane..2 * lane + 2].copy_from_slice(&code.to_le_bytes());
        }
    }
    played
}

fn compare_columns(
    (m, size): (&Model, (usize, usize)),
    p: &hz::Params,
    (whole, a): (&Settings, &[u8]),
    (window, b): (&Settings, &[u8]),
) -> (usize, usize) {
    let mut before = Frame::default();
    let mut after = Frame::default();
    let a_layout = codes_into(a, whole, p, 1, &mut before).unwrap();
    let b_layout = codes_into(b, window, p, 1, &mut after).unwrap();
    let k = seq::compress_k(window, p);
    let lo = seq::window(window, p).1;
    let shift = f64::from(lo / k);
    let anchor = |f: &Frame| f.trigger_anchor.point(f.pre);
    assert_eq!(anchor(&before), f64::from(p.pre / k));
    assert_eq!(anchor(&after), f64::from(p.pre - lo) / f64::from(k));
    if after.trigger_anchor.cropped() {
        assert_eq!(after.trigger_anchor.stored_points(after.pre) + lo as usize, p.pre as usize);
    }
    assert_eq!((before.dt.to_bits(), before.acq), (after.dt.to_bits(), after.acq));
    assert_eq!((before.seq, before.fixed_ti), (after.seq, after.fixed_ti));

    let a_plan = standard::batch_plan(m, size, &before, Some((a, a_layout)), None);
    let b_plan = standard::batch_plan(m, size, &after, Some((b, b_layout)), None);
    assert_eq!((a_plan.route, a_plan.triggered), (b_plan.route, b_plan.triggered));
    let (a_proj, a_trace) = a_plan.plan.raw().unwrap();
    let (b_proj, b_trace) = b_plan.plan.raw().unwrap();
    let translated = shift + b_trace.trigger_at;
    // Coordinates can round a fractional crossing on either side of the integer translation.
    assert!((a_trace.trigger_at - translated).abs() <= translated.abs().max(1.0) * f64::EPSILON);
    let mut envelope = 0;
    let mut batch = 0;
    for lane in 0..4 {
        let a_span = a_trace.spans[lane].as_ref();
        let b_span = b_trace.spans[lane].as_ref();
        batch += differing_spans(size.0, a_span, b_span);
        if before.envelope && a_span.is_some() {
            let a_map = kernel::raw_map(a_proj, lane, a_trace.trigger_at).unwrap();
            let b_map = kernel::raw_map(b_proj, lane, b_trace.trigger_at).unwrap();
            let values = |layout: crate::drawpipe::CodeLayout, bytes: &[u8]| {
                let codes = layout.channel(bytes, lane);
                (0..codes.len()).map(|i| codes.at(i)).collect::<Vec<_>>()
            };
            let a = crate::compress::ranges(&values(a_layout, a), &a_map, size.0);
            let b = crate::compress::ranges(&values(b_layout, b), &b_map, size.0);
            envelope += a.iter().zip(b).filter(|(left, right)| **left != *right).count();
        }
    }
    assert_eq!(seq::compress_k(whole, p), k);
    (envelope, batch)
}

fn compare_fast(
    (m, (w, h)): (&Model, (usize, usize)),
    p: &hz::Params,
    (whole, a): (&Settings, &[u8]),
    (window, b): (&Settings, &[u8]),
) -> usize {
    let frames = [(whole, a), (window, b)].map(|(s, bytes)| std::sync::Arc::new(decode(bytes, s, p, 1)));
    let plans = frames.each_ref().map(|frame| standard::fast_plan(m, w, h, frame, [false; 4]));
    assert_eq!((plans[0].route, plans[0].triggered), (plans[1].route, plans[1].triggered));
    let mut bands = [vec![0; w * (h + 1)], vec![0; w * (h + 1)]];
    let mut differences = 0;
    for lane in 0..4 {
        if frames[0].ch[lane].is_empty() {
            continue;
        }
        for ((frame, plan), band) in frames.iter().zip(&plans).zip(&mut bands) {
            band.fill(0);
            standard::add_frame_band(lane, band, 0, frame, &plan.plan);
        }
        differences +=
            (0..w).filter(|&column| (column..bands[0].len()).step_by(w).any(|i| bands[0][i] != bands[1][i])).count();
    }
    differences
}

#[test]
fn nearest_crossing_is_ranked_from_the_fractional_hardware_point() {
    let samples = [-1.0, -1.0, -1.0, -1.0, 1.0, -1.0, -1.0, -1.0];
    let trigger = crate::fastdraw::EdgeSearch { level: 0.0, hysteresis: 0.25, rising: true, either: true };
    let edge = |pre| crate::trignear::edge_near_from(&samples[..], (0, samples.len()), (pre, 8, 1), trigger).0;
    assert_eq!(edge(4.0), Some(3.5));
    assert_eq!(edge(4.0 + 1.0 / 3.0), Some(4.5));
}

fn differing_spans(width: usize, a: Option<&Spans>, b: Option<&Spans>) -> usize {
    let row = |span: Option<&Spans>, col: usize| {
        span.and_then(|span| span.rows.get(col.checked_sub(span.c0)?)).copied().unwrap_or((1, 0))
    };
    (0..width).filter(|&col| row(a, col) != row(b, col)).count()
}

fn live_10us() -> (Settings, hz::Params) {
    let s = Settings { mask: 1, tdiv: 10e-6, depth: 100_000, compress: 15, window: None, ..Settings::default() };
    let p = seq::record_geometry(&s).unwrap();
    (s, p)
}

#[test]
fn no_edge_keeps_the_crop_fraction_and_the_whole_record_floor() {
    let (s, p) = live_10us();
    let (whole, crop) = (
        Settings { window: Some((p.memsel, -i64::from(p.pre))), ..s.clone() },
        batch_play(&s, &p, 1, configured_play_window(&s, &p, 1000)),
    );
    let m = Model { tscale: super::tests::scale_at(10e-6), ..Model::default() };
    for playback in [&s, &whole, &crop] {
        let bytes = vec![0; seq::frame_bytes(playback, &p)];
        let frame = decode(&bytes, playback, &p, 1);
        let placed = standard::batch_plan(&m, (1000, 480), &frame, None, None);
        let (_, trace) = placed.plan.raw().unwrap();
        assert_eq!(trace.outcome, kernel::TrigOutcome::NotFound);
        let lo = seq::window(playback, &p).1;
        let exact = f64::from(p.pre - lo) / 15.0;
        assert_eq!(trace.trigger_at, if frame.trigger_anchor.cropped() { exact } else { exact.floor() });
        if frame.trigger_anchor.cropped() {
            let error_ns = (trace.trigger_at - trace.trigger_at.floor()) * frame.dt * 1e9;
            assert!((error_ns - 8.0).abs() < 1e-10);
        }
    }
}

#[test]
fn a_far_edge_moves_whole_record_play_but_leaves_the_crop_at_its_anchor() {
    let _scope = Scope::new();
    let (s, p) = live_10us();
    let crop = batch_play(&s, &p, 1, configured_play_window(&s, &p, 1000));
    let m = Model { tscale: super::tests::scale_at(10e-6), trig_level: 0.0, ..Model::default() };
    for play in [&s, &crop] {
        let mut frame = decode(&vec![0; seq::frame_bytes(play, &p)], play, &p, 1);
        frame.ch[0].fill(-1.0);
        frame.ch[0][frame.pre + 300..].fill(1.0);
        let (n, window, first) = crate::board::record::hw_grid(&m);
        let (ti, triggered) = crate::board::record::record_trigger(&frame, &m, n, window, first);
        let placed = fallback::direct(|| standard::batch_plan(&m, (1000, 480), &frame, None, None));
        let (_, trace) = placed.plan.raw().unwrap();
        let anchor = frame.trigger_anchor.point(frame.pre);
        if frame.trigger_anchor.cropped() {
            assert_eq!((ti, triggered), (anchor, false));
            assert_eq!((trace.trigger_at, trace.outcome), (anchor, kernel::TrigOutcome::NotFound));
            let span = trace.spans[0].as_ref().unwrap();
            assert_eq!((span.c0, span.rows.len()), (0, 1000));
            assert!(span.rows.iter().all(|&(top, bottom)| top <= bottom));
        } else {
            assert!(triggered && ti - anchor > 290.0);
            assert!(matches!(trace.outcome, kernel::TrigOutcome::Moved(distance) if distance > 290.0));
        }
    }
    assert_eq!(cropped::short_windows(), 0);
}

#[test]
fn replay_drops_the_live_window_and_stop_retains_pan_and_zoom() {
    let (s, p) = live_10us();
    let b = batch_play(&s, &p, 1, configured_play_window(&s, &p, 1000));
    assert!(seq::window(&b, &p).0 < p.memsel);
    let expected = raw_replay_settings(&s, &p, false).unwrap();
    assert_eq!(raw_replay_settings(&b, &p, false), Some(expected.clone()));
    assert_eq!(final_replay(&b, &p, 2, false, false, None), Some(expected));
    for (tdiv, delay) in [(s.tdiv, 0.0), (s.tdiv / 2.0, -30e-6), (s.tdiv / 5.0, 50e-6)] {
        assert_eq!(super::stop_plan(&s, &p, tdiv, delay), super::stop_plan(&b, &p, tdiv, delay));
    }
}

#[test]
fn invalid_window_falls_back_before_admission() {
    let _scope = Scope::new();
    let (s, mut p) = live_10us();
    p.memsel -= 1;
    let s = Settings { delay: 60e-6, ..s };
    assert!(super::live_window(&s, &p, 1000).is_err());
    let fallback = configured_play_window(&s, &p, 1000);
    assert_eq!(fallback, None);
    let b = batch_play(&s, &p, 1, fallback);
    assert_eq!((b.window, b.compress), (None, s.compress));
    assert_eq!(seq::validate_readout(&b, &p), Ok(()));
    assert_eq!(drawpipe::counters().acquisition.play_window_rejections(), 1);
}

#[test]
fn a_crop_that_covers_the_record_keeps_whole_record_play() {
    let _scope = Scope::new();
    let (s, p) = live_10us();
    let s = Settings { tdiv: 1e-9, compress: 1000, ..s };
    assert_eq!(configured_play_window(&s, &p, 1000), None);
    let f = decode(&[], &s, &p, 1);
    assert_eq!(f.trigger_anchor.point(f.pre), f64::from(p.pre / 1000));
    assert_eq!(drawpipe::counters().acquisition.play_window_rejections(), 0);
}

#[test]
fn delayed_screen_keeps_the_same_refined_edge_near_the_crop_boundary() {
    let (s, p) = live_10us();
    for (delay, exceeds_record) in [(22e-6, false), (60e-6, true)] {
        let s = Settings { delay, ..s.clone() };
        for width in [512, 1024, 1920] {
            check_delayed_screen(&s, &p, width, exceeds_record);
        }
    }
}

fn check_delayed_screen(s: &Settings, p: &hz::Params, width: usize, exceeds_record: bool) {
    let _scope = Scope::new();
    let crop = batch_play(s, p, 1, configured_play_window(s, p, width));
    let (n, lo) = seq::window(&crop, p);
    let k = seq::compress_k(&crop, p);
    let m = Model { tscale: super::tests::scale_at(10e-6), delay: s.delay as f32, ..Model::default() };
    let screen = 10.0 * s.tdiv * p.rate as f64;
    let l = crate::compress::smooth_len(f64::from(k) * width as f64 / screen);
    let near = crate::trignear::near_span(l);
    let crossing = (p.pre / k) as usize + near - l;
    let place = |play: &Settings, start: u32| {
        let mut frame = decode(&vec![0; seq::frame_bytes(play, p)], play, p, 1);
        frame.ch[0].fill(-4.0);
        frame.ch[0][crossing - (start / k) as usize..].fill(4.0);
        let placed = fallback::direct(|| standard::batch_plan(&m, (width, 480), &frame, None, None));
        let (_, trace) = placed.plan.raw().unwrap();
        assert!(
            matches!(trace.outcome, kernel::TrigOutcome::Moved(_)),
            "start={start} pre={} crossing={crossing} trigger={} outcome={:?}",
            frame.pre,
            trace.trigger_at,
            trace.outcome
        );
        trace.trigger_at + f64::from(start / k)
    };
    assert!(lo > 0 && lo + n <= p.memsel);
    let whole = place(s, 0);
    assert!((whole - crossing as f64).abs() <= l as f64, "width={width} crossing={crossing} placed={whole}");
    assert_eq!(place(&crop, lo), whole);
    let shift = (whole * f64::from(k) - f64::from(p.pre)).abs();
    assert!(shift <= f64::from(super::window::live_margin(screen, k, width)));
    assert_eq!(cropped::short_windows(), usize::from(exceeds_record));
}

#[test]
fn window_covers_the_clipped_hull_at_both_record_edges() {
    let (s, p) = live_10us();
    for (delay, left_edge) in [(-120e-6, true), (120e-6, false)] {
        let p = hz::Params { pre: if left_edge { 95_000 } else { 5_000 }, ..p };
        let s = Settings { delay, ..s.clone() };
        let b = batch_play(&s, &p, 1, configured_play_window(&s, &p, 1000));
        let (n, lo) = seq::window(&b, &p);
        let len = seq::compressed_len(&b, &p);
        assert_eq!(seq::validate_readout(&b, &p), Ok(()));
        assert_eq!((lo % 120, n % 8, len % 8, seq::frame_bytes(&b, &p) % 16), (0, 0, 0, 0));
        assert!(8 <= len && len * 15 <= n && lo + n <= p.memsel);
        let margin = super::window::live_margin(10.0 * s.tdiv * p.rate as f64, 15, 1000);
        assert!(lo + margin <= p.pre && p.pre + margin <= lo + len * 15);
        assert_eq!(if left_edge { lo } else { p.memsel - (lo + n) }, 0);
        let exact = f64::from(p.pre - lo) / 15.0;
        let frame = decode(&[], &b, &p, 1);
        assert_eq!(frame.trigger_anchor.point(frame.pre), exact);
        assert_eq!(frame.dt, 15.0 / p.rate as f64);
    }
}
