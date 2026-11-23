// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::dho::hw::header::{dispatch_fixture, for_each_fixture, Fixture};
use std::sync::mpsc::sync_channel;
use std::{ops::Range, sync::mpsc::Receiver};

mod probe;
mod report;
pub(crate) use probe::{at, note_split_dispatch, record, sample, sample_channel, sample_fold, split_dispatch_start};
use probe::{measure, Sample};
use report::Options;
pub(crate) use report::Stage;

struct Replay {
    pipe: Pipe,
    batch: Arc<Batch>,
    view: Arc<View>,
    painter: Painter,
    range: Range<usize>,
    route: Receiver<(String, Range<usize>)>,
}

impl Replay {
    fn new(fixture: &Fixture) -> Option<Self> {
        let settings = fixture.view()?;
        let (m, size, interp, zoom) = settings.inputs();
        let (workers, prior_hits) = settings.scheduling();
        let id = oos_core::change::Identity::default();
        let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
        pipe.publish_picture_view(m, size, interp, id, zoom);
        let (send, receive) = sync_channel(1);
        oos_hal::sync::lock(&pipe.workers).sender = Some(send);
        let (send, route) = sync_channel(1);
        let mut trace = route::RouteTrace::new(move |report| {
            let _ = send.send((report.route().into(), report.range()));
        });
        trace.set_replay_prior_hits(prior_hits);
        dispatch_fixture(fixture, &pipe, Some(trace));
        let Message::Batch(batch) = receive.recv().expect("fixture dispatch submits one batch") else {
            unreachable!("the fixture submits a DMA batch")
        };
        let view = oos_hal::sync::lock(&pipe.view).view.clone().expect("fixture supplies the frozen worker view");
        let range = batch.next.load(Ordering::Relaxed)..batch.frames;
        let mut painter = Painter::new(1, 0);
        painter.columns = preparation_columns::Columns::new(workers);
        Some(Self { pipe, batch, view, painter, range, route })
    }

    fn paint(&mut self) -> u64 {
        let id = self.batch.header.identity();
        self.painter.batch(None, &self.pipe.slot, &self.batch, &self.view, id).map_or(0, |deposit| deposit.records)
    }

    fn verify_route(mut self, fixture: &Fixture) {
        self.paint();
        drop(self.batch);
        let observed = self.route.recv().expect("the last batch owner reports the selected route");
        fixture.assert_route(&observed.0, &observed.1);
    }

    fn run(&mut self, options: &Options) -> Vec<Sample> {
        let view = &self.view;
        let id = self.batch.header.identity();
        let mut deposit = self
            .pipe
            .slot
            .acquire_picture(view.gen, view.picture_sizes(), id, view.m.ch.each_ref().map(|c| c.on))
            .expect("fixture picture has bounded backing");
        let (samples, reset) = measure(self.range.clone(), options, || {
            for first in self.range.clone().step_by(SLICE) {
                let count = self.painter.place_slice(&self.batch, view, first);
                if self.painter.arena[..count].iter().flatten().any(|p| {
                    p.plan.as_ref().is_some_and(|p| p.plan.format() == CountFormat::Wide)
                        || p.zoom_plan().is_some_and(|p| p.plan.format() == CountFormat::Wide)
                }) {
                    deposit.set_format(CountFormat::Wide);
                }
                self.painter.draw_tiles(&self.batch, &mut deposit, count);
            }
            Self::reset(deposit, id, options)
        });
        probe::account(samples, reset, options)
    }

    fn reset(mut deposit: Deposit, id: Option<oos_core::change::Identity>, options: &Options) -> Duration {
        if !options.measures(Stage::Reset) {
            return Duration::ZERO;
        }
        if options.repeats() == 1 {
            let started = Instant::now();
            deposit.reset(id);
            return started.elapsed();
        }
        let mut dirty: Vec<_> = deposit.planes.iter().map(DiffBand::fixture_dirty_rows).collect();
        if let Some(planes) = deposit.zoom_planes_mut() {
            dirty.extend(planes.iter().map(DiffBand::fixture_dirty_rows));
        }
        for _ in 0..options.repeats() {
            let (main, zoom) = dirty.split_at(deposit.planes.len());
            for (plane, rows) in deposit.planes.iter_mut().zip(main) {
                plane.touch(rows.clone());
            }
            if let Some(planes) = deposit.zoom_planes_mut() {
                for (plane, rows) in planes.iter_mut().zip(zoom) {
                    plane.touch(rows.clone());
                }
            }
            std::hint::black_box(&mut deposit).reset(id);
        }
        Duration::ZERO
    }
}

fn run_fixture(fixture: &Fixture, options: &Options) {
    if fixture.view().is_none() {
        eprintln!("{}: no worker view; skipping benchmark", fixture.name());
        return;
    }
    for pass in options.passes() {
        let mut replay = Replay::new(fixture).expect("worker fixture has a frozen view");
        let channels = replay.view.m.ch.each_ref().map(|ch| ch.on);
        let samples = replay.run(&pass);
        pass.report(fixture.name(), replay.range.clone(), channels, &samples);
    }
    Replay::new(fixture).expect("worker fixture has a frozen view").verify_route(fixture);
}

#[test]
#[ignore = "release benchmark for captured DMA batches"]
fn raw_batch_stage_benchmark() {
    let options = Options::from_env();
    options.pin();
    println!();
    for_each_fixture(&mut |fixture| run_fixture(fixture, &options));
}

#[test]
#[ignore = "set OOS_FIXTURES to a group of captured DMA batches"]
fn captured_fallback_counts() {
    use std::collections::BTreeMap;

    let mut records = 0;
    let mut totals = BTreeMap::new();
    for_each_fixture(&mut |fixture| {
        let mut replay = Replay::new(fixture).expect("capture has the worker view");
        let painted = replay.paint();
        assert_eq!(painted as usize, replay.range.len(), "{}", fixture.name());
        records += painted;
        for (cause, count) in replay.batch.counters.routes.fallbacks.entries() {
            if count > 0 {
                *totals.entry(cause).or_insert(0) += count;
            }
        }
    });
    print!("fallback.v1 records={records}");
    for (cause, count) in totals {
        print!(" {cause}={count}");
    }
    println!();
}

#[test]
#[ignore = "set OOS_FIXTURES to a captured short time-base group"]
fn captured_interpolated_spans_match_the_legacy_screen_grid() {
    for_each_fixture(&mut |fixture| {
        let replay = Replay::new(fixture).expect("capture has the worker view");
        let mut frame = Frame::default();
        let mut screen = std::array::from_fn(|_| Vec::new());
        let mut p = Prepared::reuse(&mut None, &replay.view, 0);
        let mut checked = 0;
        let mut max_rows = 0;
        for index in replay.range.clone() {
            p.index = index;
            let bytes = chunk(&replay.batch, index);
            let (f, placement) = placement_frame(&replay.batch, &replay.view, &mut p, &mut frame, bytes);
            if placement.route != route::Route::Upsampled || placement.words.is_none() {
                continue;
            }
            let previous = p.plan.take().and_then(|plan| plan.plan.recycle(&mut p.screen));
            finish_placement(&replay.view, &mut p, f, placement, previous);
            let plan = &p.plan.as_ref().unwrap().plan;
            let (lanes, rows) = compare_legacy_short_hull(&replay.view.m, f, plan, &mut screen);
            checked += lanes;
            max_rows = max_rows.max(rows);
        }
        assert!(checked != 0, "capture must exercise the raw interpolator");
        assert!(max_rows <= 1, "{} exceeded the row bound: {max_rows}", fixture.name());
        println!("{}: checked {checked} lane records, maximum {max_rows} row", fixture.name());
    });
}

pub(crate) fn legacy_hull(
    p: &kernel::Projection,
    channel: usize,
    (first, dt): (f64, f64),
    samples: &[f32],
) -> Vec<(i16, i16)> {
    let ch = p.ch[channel].unwrap();
    let mut rows = vec![(i16::MAX, i16::MIN); p.cols];
    crate::wave::trace_columns(
        p.cols,
        p.rows,
        (ch.volts_per_div, ch.offset),
        false,
        (p.time_per_div, p.delay),
        first,
        dt,
        samples,
        24,
        None,
        false,
        (0, p.cols as i32),
        |c, a, b, _| {
            let row = &mut rows[c as usize];
            *row = (row.0.min(a as i16), row.1.max(b as i16));
        },
    );
    rows
}

pub(crate) fn add_short_columns(ph: &mut crate::wave::Phosphor, m: &Model, f: &Frame, bytes: &[u8], raw: CodeLayout) {
    let (w, h) = (ph.w, ph.h);
    let spacing = f.dt / f64::from(m.time_per_div()) * w as f64 / f64::from(HDIV);
    let projection = standard::projection(m, w, h, spacing);
    let words = Some((bytes, raw, crate::picture::deposit::envelope::Layout::new(&raw).unwrap()));
    let placement = standard::BatchPlacement { projection, route: route::Route::Upsampled, words };
    let plan = standard::screen_plan(m, f, placement, crate::sparse::Interp::Exact, &mut Default::default(), None);
    let (lanes, rows) = compare_legacy_short_hull(m, f, &plan.plan, &mut Default::default());
    assert!(lanes > 0 && rows <= 1, "legacy hull differs by {rows} rows on {lanes} lanes");
    for (c, _) in m.ch.iter().enumerate().filter(|(_, channel)| channel.on) {
        ph.edit_channel(c, |map| {
            standard::fast_deposit(c, map, 0, f, &plan.plan);
            0..h
        });
    }
}

pub(crate) fn verify_fixture(fixture: &Fixture, stages: &[(Stage, usize)]) {
    for repeats in [1, 2] {
        for &(stage, records) in stages {
            let options = Options::for_test(stage, repeats);
            let mut replay = Replay::new(fixture).unwrap();
            let samples = replay.run(&options);
            assert_eq!(samples.iter().map(|r| r.calls(stage)).sum::<usize>(), records * repeats, "{}", stage.name());
        }
    }
    Replay::new(fixture).unwrap().verify_route(fixture);
}

fn compare_legacy_short_hull(
    m: &Model,
    frame: &Frame,
    plan: &standard::FastPlan,
    screen: &mut [Vec<f32>; NCH],
) -> (usize, u16) {
    let (projection, trace) = plan.raw().expect("the reference paints one span per column");
    let window = projection.time_per_div * f64::from(HDIV);
    let first = projection.delay - window / 2.0;
    let points = crate::sparse::POINTS;
    assert!(!crate::board::record::resample_channels(frame, m, (points, window, first), trace.trigger_at, screen));
    let (mut lanes, mut max_rows) = (0, 0);
    for (channel, (span, samples)) in trace.spans.iter().zip(screen).enumerate() {
        if projection.ch[channel].is_none() || samples.is_empty() {
            assert!(span.is_none(), "unexpected spans on lane {channel}");
            continue;
        }
        let span = span.as_ref().expect("an enabled sampled lane has spans");
        let expected = legacy_hull(projection, channel, (first, window / points as f64), samples);
        assert_eq!((span.c0, span.rows.len()), (0, expected.len()));
        for (column, (&(lo, hi), (want_lo, want_hi))) in span.rows.iter().zip(expected).enumerate() {
            assert_eq!(lo <= hi, want_lo <= want_hi, "lane {channel}, column {column}");
            if lo <= hi {
                max_rows = max_rows.max(lo.abs_diff(want_lo)).max(hi.abs_diff(want_hi));
            }
        }
        lanes += 1;
    }
    (lanes, max_rows)
}
