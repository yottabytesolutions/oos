// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::dho::hw::header::{dispatch_fixture, for_each_fixture, Fixture};
use std::sync::mpsc::sync_channel;
use std::{ops::Range, sync::mpsc::Receiver};

mod capture;
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
        Self::with_dispatch(fixture, |pipe, trace| dispatch_fixture(fixture, pipe, Some(trace)))
    }

    fn with_dispatch(fixture: &Fixture, dispatch: impl FnOnce(&Pipe, Box<route::RouteTrace>)) -> Option<Self> {
        let settings = fixture.placement_view()?;
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
        dispatch(&pipe, trace);
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

/// Whether the fixture was measured; a skipped one says why on stderr.
fn run_fixture(fixture: &Fixture, options: &Options) -> bool {
    if std::env::var_os("OOS_BENCH_AVERAGE_16").is_some() {
        return run_average_fixture(fixture, options);
    }
    let Some(mut replay) = Replay::new(fixture) else {
        return false;
    };
    for pass in options.passes() {
        let channels = replay.view.m.ch.each_ref().map(|ch| ch.on);
        let samples = replay.run(&pass);
        pass.report(fixture.name(), replay.range.clone(), channels, &samples);
        drop(replay);
        replay = Replay::new(fixture).expect("a replayed fixture replays again");
    }
    replay.verify_route(fixture);
    true
}

fn run_average_fixture(fixture: &Fixture, options: &Options) -> bool {
    if !fixture.raw_batch_samples() {
        eprintln!("{}: needs a raw DMA batch for Average 16", fixture.name());
        return false;
    }
    let Some(mut replay) = Replay::new(fixture) else { return false };
    let v = &replay.view;
    let view = Arc::new(View {
        gen: v.gen,
        m: Model { acq: AcqMode::Average, avg_count: 16, avg_hw: false, ..v.m.clone() },
        interp: v.interp,
        w: v.w,
        h: v.h,
        zoom: v.zoom,
    });
    for pass in options.passes() {
        let (samples, reset) = measure(replay.range.clone(), &pass, || {
            let (output, _) = average::average_batch(&mut replay.painter, &replay.batch, &view);
            std::hint::black_box(output.expect("the fixture needs a plain DMA grid").shown().0);
            Duration::ZERO
        });
        let samples = probe::account(samples, reset, &pass);
        pass.report(fixture.name(), replay.range.clone(), view.m.ch.each_ref().map(|ch| ch.on), &samples);
    }
    true
}

fn run_group(group: impl FnOnce(&mut dyn FnMut(&Fixture)), options: &Options) {
    let mut measured = 0;
    group(&mut |fixture| measured += usize::from(run_fixture(fixture, options)));
    assert!(measured > 0, "the fixture group has no measurable fixture: no batch submitted records");
}

#[test]
#[ignore = "release benchmark for captured DMA batches"]
fn raw_batch_stage_benchmark() {
    let options = Options::from_env();
    options.pin();
    println!();
    run_group(for_each_fixture, &options);
}

#[test]
#[ignore = "set OOS_FIXTURES to captured plain DMA batches for Average"]
fn captured_average_matches_the_decoded_grid_and_fold() {
    assert!(std::env::var_os("OOS_FIXTURES").is_some(), "set OOS_FIXTURES");
    let mut checked = 0;
    for_each_fixture(&mut |fixture| {
        if !fixture.raw_batch_samples() {
            println!("{}: the Average batch route requires raw samples", fixture.name());
            return;
        }
        let Some(replay) = Replay::new(fixture) else { return };
        checked += replay.range.len();
        average::verify_codes(fixture.name(), &replay.batch, &replay.view, replay.range);
    });
    assert!(checked > 0, "the fixture group must exercise the plain Average grid");
}

#[test]
#[ignore = "set OOS_FIXTURES to a group of captured DMA batches"]
fn captured_fallback_counts() {
    use std::collections::BTreeMap;

    let mut records = 0;
    let mut totals = BTreeMap::new();
    for_each_fixture(&mut |fixture| {
        let Some(mut replay) = Replay::new(fixture) else {
            return;
        };
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
        let Some(replay) = Replay::new(fixture) else {
            return;
        };
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

#[test]
#[ignore = "set OOS_FIXTURES to captured short time-base batches with analog edge triggers"]
fn captured_screen_refinement_has_identical_bits_when_the_shared_search_reads_codes() {
    assert!(std::env::var_os("OOS_FIXTURES").is_some(), "set OOS_FIXTURES");
    for_each_fixture(&mut |fixture| {
        let Some(replay) = Replay::new(fixture) else {
            return;
        };
        let mut scratch = Frame::default();
        let mut p = Prepared::reuse(&mut None, &replay.view, 0);
        let mut checked = 0;
        for index in replay.range.clone() {
            p.index = index;
            let bytes = chunk(&replay.batch, index);
            let (frame, placement) = placement_frame(&replay.batch, &replay.view, &mut p, &mut scratch, bytes);
            if placement.route != route::Route::Upsampled {
                continue;
            }
            if let Some((bytes, raw, _)) = placement.words {
                checked +=
                    usize::from(compare_screen_refinement(&replay.view.m, frame, placement.projection, bytes, raw));
            }
        }
        assert!(checked > 0, "{}: capture must exercise interpolated analog edge refinement", fixture.name());
    });
}

fn compare_screen_refinement(
    m: &Model,
    frame: &Frame,
    projection: kernel::Projection,
    bytes: &[u8],
    raw: CodeLayout,
) -> bool {
    use crate::board::record::smoothed_edge;
    let decoded = frame.ch[m.trig_src].as_slice();
    if m.trig_type != TrigType::Edge
        || m.trig_on_la()
        || frame.fixed_ti.is_some()
        || crate::deepmeas::deep_raw_frame(frame)
        || decoded.len() <= 1
    {
        return false;
    }
    let codes = raw.channel(bytes, m.trig_src);
    let len = frame.ch.iter().map(Vec::len).max().unwrap();
    let window = projection.time_per_div * f64::from(HDIV);
    let geometry = (crate::sparse::POINTS, window, projection.delay - window / 2.0);
    let code_position = smoothed_edge(frame, m, &codes, len, geometry, false);
    let volt_position = smoothed_edge(frame, m, decoded, len, geometry, false);
    assert_eq!(code_position.map(f64::to_bits), volt_position.map(f64::to_bits), "record {}", frame.seq);
    if let Some(position) = volt_position {
        let (placed, _) = standard::screen_trigger(m, frame, &projection, None, None);
        assert_eq!(placed.to_bits(), position.to_bits(), "record {}", frame.seq);
    }
    true
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

pub(crate) fn assert_benchmark_skips(fixture: &Fixture) {
    assert!(Replay::new(fixture).is_none(), "{}: replay setup", fixture.name());
    assert!(!run_fixture(fixture, &Options::for_test(Stage::Place, 1)), "{}: benchmark", fixture.name());
}

pub(crate) fn benchmark_group(fixtures: &[&Fixture]) {
    run_group(|each| fixtures.iter().for_each(|&fixture| each(fixture)), &Options::for_test(Stage::Place, 1));
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
