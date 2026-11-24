// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::scripted::{self, Batch as _};
use crate::system::test_allocator::count_own;

const SIZE: (usize, usize) = (970, 448);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Codes,
    Screen,
}

struct Input {
    pipe: Pipe,
    batch: Arc<Batch>,
    view: Arc<View>,
    enabled: [bool; NCH],
}

impl Input {
    fn view(channels: usize, depth: Option<usize>) -> (Model, Pipe, Arc<View>, [bool; NCH]) {
        let mut m = super::super::tests::model(1e-3, false, false);
        if let Some(depth) = depth {
            m.mdepth = mdepths().iter().position(|d| d.1 as usize == depth).unwrap();
        }
        let enabled = [channels == 2, true, false, false];
        m.trig_src = 1;
        m.meas.clear();
        m.meas2.clear();
        for (ch, on) in m.ch.iter_mut().zip(enabled) {
            ch.on = on;
        }
        let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
        pipe.set_view(Some(&m), SIZE.0, SIZE.1, Interp::Exact);
        let view = oos_hal::sync::lock(&pipe.view).view.clone().unwrap();
        (m, pipe, view, enabled)
    }

    fn planned_10m() -> (Self, String) {
        let (m, pipe, view, enabled) = Self::view(1, Some(10_000_000));
        // The board's unrestricted plan can hold more records. Serial admission grants one.
        let (input, frames, geometry) = scripted::draw_route_batch(&m, &Default::default(), 1);
        assert_eq!(frames, 1);
        let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
        let newest = Arc::new(input.newest(m.hw_stamp));
        assert_eq!(newest.acq.1, 10_000_000);
        assert!(newest.envelope);
        let mut batch = pipe.feed_batch(Box::new(input.into_bytes()), fb, 0..frames, (0, None), decode, codes);
        Arc::get_mut(&mut batch).unwrap().newest = Some(newest);
        (Self { pipe, batch, view, enabled }, geometry)
    }

    fn new(points: usize, channels: usize, shape: Shape) -> Self {
        let (_, pipe, view, enabled) = Self::view(channels, None);
        let mask = if channels == 1 { 2 } else { 3 };
        let cal = [(1.0, 0.0); NCH];
        let rate = 1.25e9 / channels as f64;
        let depth = if shape == Shape::Screen { points.max(4_000_000) } else { points.min(1_000_000) };
        let set_metadata = move |f: &mut Frame| {
            f.dt = rate.recip();
            f.pre = points / 2;
            f.pre_exact = true;
            f.acq = (rate, depth);
        };
        let decode: DecodeInto = Arc::new(move |bytes, f| {
            oos_hal::dho::seq::decode_volts_into(bytes, mask, cal, &mut f.ch);
            set_metadata(f);
        });
        let codes = (shape == Shape::Codes).then(|| {
            let map = oos_hal::dho::seq::code_map(mask, cal);
            Arc::new(move |_, _: &[u8], f: &mut Frame| {
                set_metadata(f);
                let mut layout = CodeLayout::uncorrected(map.ratio, map.word, map.affine, oos_hal::dho::seq::volt_code);
                layout.corrected = Some(
                    [crate::fastdraw::CorrectedCodes {
                        offsets: [-13, 31, -7, 6, 14, -3, 0, 15],
                        scale: 1.0,
                        offset: 0.0,
                    }; NCH],
                );
                Some(layout)
            }) as CodesInto
        });
        let bytes = signal(points, channels);
        let fb = bytes.len();
        let batch = pipe.feed_batch(Box::new(super::super::tests::Bytes(bytes)), fb, 0..1, (0, None), decode, codes);
        Self { pipe, batch, view, enabled }
    }

    fn deposit(&self) -> Deposit {
        self.pipe.slot.acquire(self.view.gen, SIZE, None, self.enabled).unwrap()
    }

    fn paint(&self, painter: &mut Painter, deposit: &mut Deposit) {
        painter.cost = metrics::Work::default();
        deposit.reset(None);
        let count = painter.place_slice(&self.batch, &self.view, 0);
        painter.draw_tiles(&self.batch, deposit, count);
        deposit.sort_bands();
    }
}

fn signal(points: usize, channels: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(points * channels * size_of::<u16>());
    for index in 0..points {
        for lane in 0..channels {
            let cycle = (index + lane * 79) % 901;
            let edge = if cycle < 450 { 28_000 } else { 37_000 };
            let value = (edge + (index % 137) * 17) as u16;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes
}

#[test]
fn deep_column_placement_matches_unsplit_planes() {
    for points in [1_000_000, 4_000_000, 10_000_000] {
        for channels in [1, 2] {
            for shape in [Shape::Codes, Shape::Screen] {
                let input = Input::new(points, channels, shape);
                check_planes(&input, &format!("{shape:?}, {points} samples, {channels} channels"));
            }
        }
    }
    let (input, geometry) = Input::planned_10m();
    check_planes(&input, &geometry);
}

fn check_planes(input: &Input, geometry: &str) {
    let mut unsplit = Painter::new(1, 0);
    let mut split = Painter::new(4, 0);
    let mut expected = input.deposit();
    let mut actual = input.deposit();
    input.paint(&mut unsplit, &mut expected);
    input.paint(&mut split, &mut actual);
    assert_eq!(split.columns.task_allocations().count(), 4, "{geometry}");
    assert_eq!(expected.planes.len(), 4, "{geometry}");
    assert_eq!(expected.planes.len(), actual.planes.len(), "{geometry}");
    for (a, b) in expected.planes.iter().zip(&actual.planes) {
        assert_eq!((a.channel, a.first), (b.channel, b.first));
        assert_eq!(a.plane.d, b.plane.d, "{geometry}");
    }
    assert!(actual.planes.iter().flat_map(|band| &band.plane.d).any(|&v| v != 0), "{geometry}");
}

#[test]
fn warmed_deep_column_placement_allocates_nothing() {
    for shape in [Shape::Codes, Shape::Screen] {
        let input = Input::new(1_000_000, 2, shape);
        let mut painter = Painter::new(4, 0);
        let place = |painter: &mut Painter| painter.place_slice(&input.batch, &input.view, 0);
        place(&mut painter);
        for _ in 0..4 {
            let (allocations, count) = count_own(|| place(&mut painter));
            assert_eq!(count, 1);
            // A sync_channel allocates its waiter storage the first time it blocks. Which
            // invocation first blocks depends on scheduling, so count transport separately.
            assert_eq!(allocations - painter.columns.dispatch_allocations(), 0, "{shape:?} coordinator");
            assert_eq!(painter.columns.task_allocations().collect::<Vec<_>>(), [0; 4], "{shape:?} workers");
        }
    }
}

#[test]
#[ignore = "release host measurement"]
fn deep_column_pool_wall() {
    let (planned, geometry) = Input::planned_10m();
    benchmark(planned, &geometry);
    benchmark(Input::new(10_000_000, 1, Shape::Codes), "10 M transferred codes (direct envelope)");
    benchmark(Input::new(10_000_000, 1, Shape::Screen), "10 M transferred codes (screen resample)");
}

fn benchmark(input: Input, geometry: &str) {
    for split in [false, true] {
        let mut painter = Painter::new(4, 0);
        if !split {
            painter.columns = preparation_columns::Columns::new(1);
        }
        let mut deposit = input.deposit();
        for _ in 0..6 {
            input.paint(&mut painter, &mut deposit);
        }
        assert_eq!(painter.columns.task_allocations().count(), if split { 4 } else { 0 });
        let mut samples = [[Duration::ZERO; 31]; 3];
        let [wall, tiles, refine] = &mut samples;
        for ((wall, tiles), refine) in wall.iter_mut().zip(tiles).zip(refine) {
            input.paint(&mut painter, &mut deposit);
            *wall = painter.cost.pool;
            *tiles = painter.cost.tiles;
            *refine = painter.cost.refine;
        }
        for series in &mut samples {
            series.sort_unstable();
        }
        let ms = |sample: Duration| sample.as_secs_f64() * 1e3;
        println!(
            "{geometry}, split={split}: pool median {:.3} ms, p90 {:.3} ms, summed tiles median {:.3} ms, reported refine median {:.3} ms",
            ms(samples[0][15]), ms(samples[0][27]), ms(samples[1][15]), ms(samples[2][15]),
        );
    }
}

fn trigger_fixture(offset: usize) -> Input {
    let (mut input, _) = Input::planned_10m();
    let batch = Arc::get_mut(&mut input.batch).unwrap();
    batch.codes = None;
    let frame = Arc::get_mut(batch.newest.as_mut().unwrap()).unwrap();
    assert_eq!(frame.acq, (625_000_000.0, 10_000_000));
    assert_eq!((frame.pre, frame.ch[1].len()), (312_500, 625_000));
    assert_eq!(frame.dt, 16.0 / frame.acq.0);
    assert_eq!(offset % 2, 0);
    let hyst = input.view.m.trig_hyst_div() * input.view.m.ch[1].volts_per_div();
    let edge = frame.pre + offset;
    // A valid rising trigger can leave the high side inside the falling arm rail.
    frame.ch[1][..edge].fill(-2.0 * hyst);
    frame.ch[1][edge..].fill(0.5 * hyst);
    input
}

fn serial_trigger(input: &Input) -> (f64, bool) {
    let frame = input.batch.newest.as_ref().unwrap();
    let (projection, _) = route::projection(&input.view, frame.dt);
    let (ti, outcome) = standard::screen_trigger(&input.view.m, frame, &projection.main(), None, None);
    (ti, outcome != kernel::TrigOutcome::NotFound)
}

fn check_pair_trigger(offset: usize, expected_wide: u64) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (mut reference, input) = (trigger_fixture(offset), trigger_fixture(offset));
    let routes = &crate::drawpipe::counters().routes;
    let expected = 312_532.399_999_979_9 + offset as f64;
    assert_eq!(serial_trigger(&input), (expected, true));
    assert_eq!(routes.wide_searches(), expected_wide);

    let batch = Arc::get_mut(&mut reference.batch).unwrap();
    Arc::get_mut(batch.newest.as_mut().unwrap()).unwrap().fixed_ti = Some(expected);
    let mut expected = reference.deposit();
    reference.paint(&mut Painter::new(1, 0), &mut expected);
    let mut actual = input.deposit();
    let mut painter = Painter::new(4, 0);
    input.paint(&mut painter, &mut actual);
    assert_eq!(routes.wide_searches(), 2 * expected_wide);
    let plan = painter.arena[0].as_ref().unwrap().plan.as_ref().unwrap();
    assert!(plan.triggered);
    assert_eq!(plan.route, route::Route::Pairs);
    assert_eq!(actual.planes.len(), expected.planes.len());
    for (a, b) in actual.planes.iter().zip(&expected.planes) {
        assert_eq!((a.channel, a.first, &a.plane.d), (b.channel, b.first, &b.plane.d));
    }
    assert!(actual.planes.iter().flat_map(|band| &band.plane.d).any(|&v| v != 0));

    // Trigger and envelope dispatch share the same warmed jobs, including the worker sample buffers.
    let (allocations, count) = count_own(|| painter.place_slice(&input.batch, &input.view, 0));
    assert_eq!(count, 1);
    assert_eq!(allocations - painter.columns.dispatch_allocations(), 0);
    assert_eq!(painter.columns.task_allocations().collect::<Vec<_>>(), [0; 4]);
}

#[test]
fn deep_pairs_centred_trigger_uses_the_narrow_window() {
    check_pair_trigger(0, 0);
}

#[test]
fn deep_pairs_wide_trigger_preserves_the_serial_picture() {
    check_pair_trigger(80_000, 1);
}

#[test]
fn a_sub_column_trigger_shift_can_move_a_sample_across_a_column_boundary() {
    let mut m = super::super::tests::model(1e-3, false, false);
    m.ch[1].on = true;
    m.trig_src = 0;
    let size = 64;
    let spacing = 1.0 / 64.0;
    let dt = f64::from(m.time_per_div()) * f64::from(HDIV) * spacing / size as f64;
    let mut projection = standard::projection(&m, size, size, spacing);
    projection.dots = true;
    let mut frame = Frame { pre: 2048, dt, pre_exact: true, ..Default::default() };
    frame.ch[0].resize(4096, -1.0);
    frame.ch[0][frame.pre..].fill(63.0 / 64.0);
    frame.ch[1].resize(4096, 0.0);
    frame.ch[1][frame.pre] = 0.75;
    let render = |f: &Frame| {
        let placement = standard::BatchPlacement { projection, route: route::Route::Dots, words: None };
        let plan = standard::batch_plan_timed(&m, f, placement, None, None);
        assert!(plan.triggered);
        let mut counts = vec![0; size * size];
        standard::fast_deposit(1, &mut counts, 0, f, &plan.plan);
        (plan.plan.raw().unwrap().1.trigger_at, counts)
    };
    let (ti, refined) = render(&frame);
    let columns = (ti - frame.pre as f64) * spacing;
    assert!(columns > 0.0 && columns < 0.01);
    frame.fixed_ti = Some(frame.pre as f64);
    assert_ne!(refined, render(&frame).1);
}

#[test]
#[ignore = "release host measurement"]
fn deep_pairs_trigger_refine() {
    for offset in [0, 80_000] {
        let input = trigger_fixture(offset);
        let answer = serial_trigger(&input);
        let mut painter = Painter::new(4, 0);
        let mut deposit = input.deposit();
        for _ in 0..6 {
            input.paint(&mut painter, &mut deposit);
        }
        let mut serial = [Duration::ZERO; 51];
        let mut pooled = serial;
        for (one, four) in serial.iter_mut().zip(&mut pooled) {
            let started = Instant::now();
            std::hint::black_box(serial_trigger(std::hint::black_box(&input)));
            *one = started.elapsed();
            input.paint(&mut painter, &mut deposit);
            *four = painter.cost.refine;
        }
        for (name, mut samples) in [("serial", serial), ("pooled", pooled)] {
            samples.sort_unstable();
            println!(
                "10 M CH2 pairs, edge {offset:+} words from pre: {answer:?}, {name} refine median {:?}, p90 {:?}",
                samples[25], samples[45]
            );
        }
    }
}
