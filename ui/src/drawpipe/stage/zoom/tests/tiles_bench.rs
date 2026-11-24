// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const RECORDS: usize = 16;

#[test]
#[ignore = "release host tile benchmark"]
fn zoom_tiles_report() {
    let m = model(NCH);
    let (b, pipe) = fixture(&m);
    println!("4 workers, {RECORDS} records/slice; all times in usec/record");
    for (factor, tdiv) in [(0, None), (10, Some(1e-6)), (200, Some(50e-9)), (2000, Some(5e-9))] {
        let v = view(m.clone(), tdiv);
        report(factor, &v, &b, &pipe, false);
        if v.zoom.is_some() {
            report(factor, &v, &b, &pipe, true);
        }
    }
}

fn fixture(m: &Model) -> (Arc<Batch>, Pipe) {
    let switches = crate::sysconf::Switches { compress: false, ..Default::default() };
    let (input, frames, _) = scripted::draw_route_batch(m, &switches, RECORDS);
    let sample = input.frame(0, 0);
    assert_eq!((frames, sample.acq.0, sample.ch[0].len()), (RECORDS, 312_500_000.0, 35_160));
    assert_eq!(m.acq, AcqMode::Normal);
    let (fb, decode, codes) = (input.frame_bytes(), input.decode(), input.codes());
    let pipe = Pipe::start(0, 0, Box::leak(Box::default()));
    let b = pipe.feed_batch(Box::new(input.into_bytes()), fb, 0..frames, (0, None), decode, codes);
    (b, pipe)
}

const WARMUP: usize = 8;

#[test]
fn zoom_tiles_reuse_the_four_workers_scratch() {
    let m = model(NCH);
    let (b, pipe) = fixture(&m);
    let v = view(m, Some(5e-9));
    let mut painter = Painter::new(4, 0);
    painter.pool = Pool::new(4, 0, run_counted);
    let mut deposit = pipe.slot.acquire_picture(v.gen, v.picture_sizes(), None, [true; NCH]).unwrap();
    let mut paint = || {
        let count = painter.place_slice(&b, &v, 0);
        painter.draw_tiles(&b, &mut deposit, count);
        deposit.reset(None);
    };
    for _ in 0..WARMUP {
        paint();
    }
    let (allocations, ()) = crate::system::test_allocator::count_own(paint);
    assert_eq!(allocations, 0);
}

fn run_counted(scratch: &mut Frame, job: &mut Job) {
    // Each worker places four records and paints one channel of each map per pass.
    const WARMUP_JOBS: usize = WARMUP * (RECORDS / 4 + 2);
    thread_local! {
        static COMPLETED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }
    let (allocations, ()) = crate::system::test_allocator::count_own(|| run(scratch, job));
    COMPLETED.with(|completed| {
        if completed.get() >= WARMUP_JOBS {
            assert_eq!(allocations, 0);
        }
        completed.set(completed.get() + 1);
    });
}

fn report(factor: usize, v: &Arc<View>, b: &Arc<Batch>, pipe: &Pipe, legacy: bool) {
    let mut painter = Painter::new(4, 0);
    if legacy {
        painter.pool = Pool::new(4, 0, run_reference);
    }
    let mut deposit = pipe.slot.acquire_picture(v.gen, v.picture_sizes(), None, [true; NCH]).unwrap();
    let [place, tile, cpu, clear] = time(|| {
        let started = Instant::now();
        black_box(painter.place_slice(b, v, 0));
        let placement = started.elapsed();
        painter.cost = metrics::Work::default();
        painter.draw_tiles(b, &mut deposit, RECORDS);
        let (tile, cpu) = (painter.cost.pool, painter.cost.tiles);
        let started = Instant::now();
        deposit.reset(None);
        [placement, tile, cpu, started.elapsed()]
    });
    println!(
        "{factor:4}x {:9} placement {place:7.2}, tile wall {tile:7.2}, tile CPU {cpu:7.2}, reset {clear:5.2}, total {:7.2}",
        if legacy { "reference" } else { "current" },
        place + tile
    );
    pipe.slot.recycle(deposit);
}

fn time(mut sample: impl FnMut() -> [Duration; 4]) -> [f64; 4] {
    const RUNS: usize = 100;
    for _ in 0..4 {
        sample();
    }
    let groups: [[Duration; 4]; 7] = std::array::from_fn(|_| {
        let mut sums = [Duration::ZERO; 4];
        for _ in 0..RUNS {
            for (sum, value) in sums.iter_mut().zip(sample()) {
                *sum += value;
            }
        }
        sums
    });
    std::array::from_fn(|i| {
        let mut sorted = groups.map(|group| group[i]);
        sorted.sort_unstable();
        sorted[sorted.len() / 2].as_secs_f64() * 1e6 / (RECORDS * RUNS) as f64
    })
}

fn run_reference(scratch: &mut Frame, job: &mut Job) {
    let Job::Tile(_, placed, (count, zoom), band, elapsed) = job else {
        run(scratch, job);
        return;
    };
    let started = Instant::now();
    for p in placed[..*count].iter().flatten() {
        let plan = if *zoom { p.zoom_plan() } else { p.plan.as_ref() };
        let Some(plan) = plan else { continue };
        // This fixture always projects already-resampled Normal screens for both maps.
        let rows = standard::add_sample_band(
            band.channel,
            &mut band.plane.d,
            band.first,
            kernel::SamplesRef::Volts(&[]),
            &plan.plan,
        );
        band.touch(rows);
    }
    *elapsed = started.elapsed();
}
