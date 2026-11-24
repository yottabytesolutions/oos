// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{
    board::record,
    drawpipe::route::Route,
    fastdraw::EdgeSearch,
    model::{ModelExt, Slope, TrigCoup},
    trignear::{edge_near_cropped, edge_near_from},
};
use std::path::PathBuf;

#[test]
fn captured_crops_place_at_the_near_result_without_short_windows() {
    let paths = recorded_crops();
    let mut failures = Vec::new();
    for path in paths {
        let fixture = Fixture::load(&path).unwrap();
        let report = replay(&fixture);
        println!(
            "{}: {} records, DMA/decoded short {:?}, differs from near {:?}",
            fixture.name(),
            report.records,
            report.short,
            report.off_near
        );
        if report.short != [0; 2] || report.off_near != [0; 2] {
            failures.push(fixture.name().to_owned());
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}

#[test]
fn a_filtered_trigger_without_a_software_crossing_keeps_the_hardware_point() {
    let fixture = Fixture::load(&recorded_crops()[0]).unwrap();
    let (s, p) = fixture.settings();
    let (_, submitted) = fixture.expected_route().unwrap();
    let f = decode(fixture.records().nth(submitted.start).unwrap(), s, p, 1);
    let anchor = f.trigger_anchor.point(f.pre);
    let mut m = fixture.view().unwrap().inputs().0.clone();
    m.trig_level += 100.0;
    let (n, window, first) = record::hw_grid(&m);
    // The comparator triggered on the filtered signal. Only DC coupling lets the software level contradict it.
    for (coupling, triggered) in [(TrigCoup::Ac, true), (TrigCoup::HfReject, true), (TrigCoup::Dc, false)] {
        m.trig_coup = coupling;
        assert_eq!(record::record_trigger(&f, &m, n, window, first), (anchor, triggered), "{coupling:?}");
    }
}

fn recorded_crops() -> Vec<PathBuf> {
    if let Some(root) = std::env::var_os("OOS_CROPPED_PLAY_FIXTURES") {
        let mut paths: Vec<_> = std::fs::read_dir(root).unwrap().map(|entry| entry.unwrap().path()).collect();
        paths.sort();
        assert!(!paths.is_empty());
        paths
    } else {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/played-window/cropped");
        ["batch-024", "batch-025", "batch-030"].map(|name| root.join(name)).into()
    }
}

#[derive(Default)]
struct Report {
    records: usize,
    short: [usize; 2],
    off_near: [usize; 2],
}

fn replay(fixture: &Fixture) -> Report {
    let _scope = Scope::new();
    let (s, p) = fixture.settings();
    let (m, size, _, _) = fixture.view().unwrap().inputs();
    let trigger = EdgeSearch {
        level: m.trig_level,
        hysteresis: m.trig_hyst_div() * m.ch[m.trig_src].volts_per_div(),
        rising: m.slope != Slope::Fall,
        either: m.slope == Slope::Either,
    };
    let (_, submitted) = fixture.expected_route().unwrap();
    let records = submitted.len();
    assert!(records != 0);
    let (n, window, first) = record::hw_grid(m);
    let k = seq::compress_k(s, p);
    let margin = super::super::window::live_margin(window * p.rate as f64, k, size.0);
    let mut report = Report { records, ..Report::default() };
    for bytes in fixture.records().skip(submitted.start).take(records) {
        let f = std::sync::Arc::new(decode(bytes, s, p, 1));
        assert!(f.trigger_anchor.cropped() && !f.trigger_anchor.at_adc_rate());
        let anchor = f.trigger_anchor.point(f.pre);
        let mut dma = Frame::default();
        let layout = codes_into(bytes, s, p, 1, &mut dma).unwrap();
        let placed = fallback::direct(|| standard::batch_plan(m, size, &dma, Some((bytes, layout)), None));
        let after_dma = short_windows();
        report.short[0] = after_dma - report.short[1];
        let (projection, trace) = placed.plan.raw().unwrap();
        assert_eq!(placed.route, Route::Pairs);
        let codes = layout.channel(bytes, m.trig_src);
        let col = crate::compress::smooth_len(projection.x_per_sample);
        let dma_search = (anchor, (window / 2.0 / f.dt) as usize, col);
        let dma_near = edge_near_from(&codes, (0, codes.len()), dma_search, trigger).0.unwrap_or(anchor);
        let col = (window / n as f64 / f.dt).floor().clamp(1.0, 256.0) as usize;
        let decoded_span = (window / f.dt).ceil() as usize / 2;
        let decoded_near =
            edge_near_cropped(f.ch[m.trig_src].as_slice(), (anchor, decoded_span, col), anchor + first / f.dt, trigger);
        let (ti, triggered) = record::record_trigger(&f, m, n, window, first);
        report.off_near[0] += usize::from(trace.trigger_at != dma_near);
        report.off_near[1] += usize::from((ti, triggered) != (decoded_near.unwrap_or(anchor), decoded_near.is_some()));
        assert!((dma_near - anchor).abs() * f64::from(k) <= f64::from(margin));
        assert!((decoded_near.unwrap_or(anchor) - anchor).abs() * f64::from(k) <= f64::from(margin));
        standard::fast_plan(m, size.0, size.1, &f, [false; 4]);
        report.short[1] += short_windows() - after_dma;
    }
    report
}

pub(super) fn short_windows() -> usize {
    let state = drawpipe::counters().pipeline_text(std::time::Duration::ZERO, 0);
    state.split_ascii_whitespace().find_map(|field| field.strip_prefix("window.short=")).unwrap().parse().unwrap()
}
