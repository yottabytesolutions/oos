// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{
    dho::hw::header::{dispatch_fixture, for_each_fixture, Fixture},
    Frame,
};
use crate::drawpipe::{self, route::RouteTrace, stage::owner::Owner, Pipe, Stats};
use crate::model::{tscales, HDIV};
use crate::picture::PerMap;
use crate::ui::lifecycle::test_wait::HANG_GUARD;
use crate::wave::Phosphor;
use oos_core::change::Identity;
use std::{
    cell::Cell,
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc, Mutex,
    },
    time::Instant,
};

const COLLECT: u8 = 1;
const BYPASS: u8 = 2;
static MODE: AtomicU8 = AtomicU8::new(0);
static TOTAL: Mutex<Comparison> = Mutex::new(Comparison {
    exact: 0,
    fallback: 0,
    different: None,
    anchored: 0,
    anchored_different: 0,
    shifts: Vec::new(),
});

thread_local! {
    static SMOOTHED: Cell<Option<f64>> = const { Cell::new(None) };
    static IN_SPARSE: Cell<bool> = const { Cell::new(false) };
}

pub(super) enum Outcome {
    Refined(bool, SearchResult),
    Anchored(SearchResult),
}

#[derive(Default)]
struct Comparison {
    exact: usize,
    fallback: usize,
    different: Option<(SearchResult, SearchResult)>,
    anchored: usize,
    anchored_different: usize,
    shifts: Vec<(f64, f64)>,
}

pub(super) fn compare<S: TriggerSamples + ?Sized>(
    source: &S,
    (pre, span, width): (usize, usize, usize),
    trigger: EdgeSearch,
    fits: &dyn Fn(f64) -> bool,
    outcome: Outcome,
) {
    if MODE.load(Ordering::Relaxed) != COLLECT {
        return;
    }
    let wide = smooth_all(source, pre, trigger, span, width, fits);
    let bits = |(want, any): SearchResult| (want.map(f64::to_bits), any.map(|e| (e.t.to_bits(), e.rising)));
    let mut total = oos_hal::sync::lock(&TOTAL);
    match outcome {
        Outcome::Refined(bounded, result) => {
            SMOOTHED.set(result.0);
            if bits(result) != bits(wide) {
                total.different.get_or_insert((result, wide));
            }
            total.exact += usize::from(bounded);
            total.fallback += usize::from(!bounded);
        }
        Outcome::Anchored(result) => {
            total.anchored += 1;
            total.anchored_different += usize::from(bits(result) != bits(wide));
        }
    }
}

pub(crate) fn bypass() -> bool {
    MODE.load(Ordering::Relaxed) == BYPASS
}

pub(crate) fn placed(frame: &Frame, ti: f64) {
    if MODE.load(Ordering::Relaxed) == COLLECT {
        let samples = (ti - frame.pre as f64).abs();
        oos_hal::sync::lock(&TOTAL).shifts.push((samples, samples * frame.dt));
    }
}

pub(crate) fn sparse(frame: &Frame, project: impl FnOnce() -> Option<bool>) -> Option<bool> {
    SMOOTHED.set(None);
    IN_SPARSE.set(true);
    let result = project();
    IN_SPARSE.set(false);
    if result.is_some() {
        placed(frame, SMOOTHED.get().unwrap_or(frame.pre as f64));
    }
    result
}

pub(super) fn bypass_sparse() -> bool {
    bypass() && IN_SPARSE.get()
}

fn histogram(shifts: impl Iterator<Item = f64>) -> (usize, BTreeMap<i32, usize>, f64) {
    let mut bins = BTreeMap::new();
    let mut zero = 0;
    let mut max = 0.0f64;
    for d in shifts {
        max = max.max(d);
        if d == 0.0 {
            zero += 1;
        } else {
            *bins.entry(d.log2().floor() as i32).or_default() += 1;
        }
    }
    (zero, bins, max)
}

fn replay(fixture: &Fixture, mode: u8) -> Option<PerMap<Phosphor>> {
    let view = fixture.view()?;
    let (model, size, interp, zoom) = view.inputs();
    let (workers, prior_hits) = view.scheduling();
    let pipe = Pipe::start(workers, 0, Box::leak(Box::default()));
    pipe.publish_picture_view(model, size, interp, Identity::default(), zoom);
    let (send, receive) = mpsc::channel();
    let mut trace = RouteTrace::new(move |report| send.send((report.route().to_owned(), report.range())).unwrap());
    trace.set_replay_prior_hits(prior_hits);
    let mut main = Phosphor::new(size.0, size.1);
    let mut detail = zoom.map(|z| {
        let (w, h) = z.size();
        Phosphor::new(w, h)
    });

    MODE.store(mode, Ordering::Relaxed);
    dispatch_fixture(fixture, &pipe, Some(trace));
    drawpipe::host::finish_batch(&pipe, 0).unwrap();
    let (route, range) = receive.recv().unwrap();
    MODE.store(0, Ordering::Relaxed);
    let accepted = Owner::default().compose_picture(
        &pipe,
        PerMap::new(&mut main, detail.as_mut()),
        &mut crate::picture::FirstDraw::Minimum,
        Stats { edges: &mut [0; 4], off: &mut (0.0, 0.0) },
        None,
    );
    assert_eq!(drawpipe::shutdown::close(&pipe, Instant::now() + HANG_GUARD), 0);
    assert!(accepted.frames > 0, "{} did not publish a picture", fixture.name());
    fixture.assert_route(&route, &range);
    if mode == COLLECT {
        assert_eq!(oos_hal::sync::lock(&TOTAL).shifts.len(), range.len(), "{}", fixture.name());
    }
    Some(PerMap::new(main, detail))
}

fn compare_planes(a: &Phosphor, b: &Phosphor) -> (usize, u64) {
    assert_eq!((a.w, a.h), (b.w, b.h));
    (0..4).flat_map(|c| (0..a.w * a.h).map(move |i| (a.count(c, i), b.count(c, i)))).fold(
        (0, 0xcbf2_9ce4_8422_2325),
        |(different, hash), (a, b)| {
            (different + usize::from(a != b), (hash ^ u64::from(a)).wrapping_mul(0x0100_0000_01b3))
        },
    )
}

fn report(fixture: &Fixture) {
    let Some(refined) = replay(fixture, COLLECT) else { return };
    let bypassed = replay(fixture, BYPASS).unwrap();
    let (model, (cols, _), _, zoom) = fixture.view().unwrap().inputs();
    let (main, detail) = refined.into_parts();
    let (base_main, base_detail) = bypassed.into_parts();
    let total = std::mem::take(&mut *oos_hal::sync::lock(&TOTAL));
    assert_eq!(total.different, None, "{}: first (production, wide) difference", fixture.name());
    let columns_per_second = |w: usize, tdiv: f32| w as f64 / (f64::from(HDIV) * f64::from(tdiv));
    let main_rate = columns_per_second(cols, model.time_per_div());
    let detail_rate = zoom.map(|z| columns_per_second(z.size().0, tscales()[model.zscale]));
    let samples = histogram(total.shifts.iter().map(|&(samples, _)| samples));
    let columns = |rate| histogram(total.shifts.iter().map(|&(_, seconds)| seconds * rate));
    let (main_changed, main_hash) = compare_planes(&main, &base_main);
    let detail = detail.zip(base_detail).map(|(a, b)| compare_planes(&a, &b));
    println!("counts {} {:?}", fixture.name(), (main_hash, detail.map(|(_, hash)| hash)));
    println!(
        "{} records={} exact={} fallback={} anchored={} anchored_different={}",
        fixture.name(),
        total.shifts.len(),
        total.exact,
        total.fallback,
        total.anchored,
        total.anchored_different,
    );
    println!(
        "hist {} samples={samples:?} main={:?} detail={:?} changed_cells={}:{}",
        fixture.name(),
        columns(main_rate),
        detail_rate.map(columns),
        main_changed,
        detail.map_or(0, |(changed, _)| changed),
    );
}

#[test]
#[ignore = "capture replay: set OOS_FIXTURES to a fixture group"]
fn captured_trigger_positions_match_the_wide_search() {
    assert!(std::env::var_os("OOS_FIXTURES").is_some(), "set OOS_FIXTURES");
    for_each_fixture(&mut report);
}
