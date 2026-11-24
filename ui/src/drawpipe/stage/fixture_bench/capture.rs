// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::hash::{DefaultHasher, Hash, Hasher};

const ROUNDS: usize = 31;

#[test]
#[ignore = "set OOS_FIXTURES to captured Peak or full-rate DMA batches"]
fn capture_profile_worker_benchmark() {
    let mut measured = 0;
    for_each_fixture(&mut |fixture| {
        let mut prepare = Duration::ZERO;
        let (profile, expected) = if fixture.raw_batch_samples() {
            ("reference", route::Route::Volts)
        } else {
            ("peak", route::Route::Pairs)
        };
        let mut replay = Replay::with_dispatch(fixture, |pipe, trace| {
            prepare = fixture.dispatch_capture_profile(pipe, trace);
        })
        .expect("capture fixture reached placement");
        benchmark(fixture, &mut replay, profile, expected, prepare);
        if fixture.raw_batch_samples() {
            let mut replay = Replay::new(fixture).expect("full-rate fixture reached placement");
            benchmark(fixture, &mut replay, "full", route::Route::Dense, Duration::ZERO);
        }
        measured += 1;
    });
    assert!(measured > 0, "set OOS_FIXTURES to capture fixture directories");
}

fn benchmark(fixture: &Fixture, replay: &mut Replay, profile: &str, expected: route::Route, prepare: Duration) {
    let mut first = paint(replay);
    assert_eq!(replay.pipe.counters.routes.count(expected), first.records);
    let picture = fingerprint(&mut first, &replay.view);
    replay.pipe.slot.recycle(first);

    let mut wall = [Duration::ZERO; ROUNDS];
    let mut work = [Duration::ZERO; ROUNDS];
    for (wall, work) in wall.iter_mut().zip(&mut work) {
        let started = Instant::now();
        let deposit = paint(replay);
        *wall = started.elapsed();
        *work = replay.painter.cost.refine + replay.painter.cost.envelope + replay.painter.cost.tiles;
        replay.pipe.slot.recycle(deposit);
    }
    wall.sort_unstable();
    work.sort_unstable();
    let records = replay.range.len() as u32;
    println!(
        "{} {profile} records={records} wall_ns={} worker_ns={} source_prepare_ns={} picture={picture:016x}",
        fixture.name(),
        (wall[ROUNDS / 2] / records).as_nanos(),
        (work[ROUNDS / 2] / records).as_nanos(),
        prepare.as_nanos(),
    );
}

fn paint(replay: &mut Replay) -> Deposit {
    let batch = &replay.batch;
    batch.next.store(replay.range.start, Ordering::Relaxed);
    let deposit = replay
        .painter
        .batch(None, &replay.pipe.slot, batch, &replay.view, batch.header.identity())
        .expect("the capture fixture paints a deposit");
    assert_eq!(deposit.records as usize, replay.range.len());
    deposit
}

fn fingerprint(deposit: &mut Deposit, view: &View) -> u64 {
    let mut picture = Phosphor::new(view.w, view.h);
    for plane in &mut deposit.planes {
        plane.fold_picture([None, None], &mut picture);
    }
    let mut hash = DefaultHasher::new();
    picture.maps().hash(&mut hash);
    hash.finish()
}
