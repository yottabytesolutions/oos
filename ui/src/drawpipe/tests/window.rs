// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::time::{Duration, Instant};

struct Fixture {
    pipe: Pipe,
    window: DepositWindow,
    picture: Phosphor,
    origin: Instant,
    edges: [u64; 4],
}

impl Fixture {
    fn new() -> Self {
        Self::with_height(4)
    }

    fn with_height(h: usize) -> Self {
        let pipe = Pipe::start(0, 0, fresh());
        pipe.set_view(Some(&Model::default()), 4, h, Interp::Exact);
        Self { pipe, window: Default::default(), picture: Phosphor::new(4, h), origin: Instant::now(), edges: [0; 4] }
    }

    fn put(&self, records: u64, hits: u64, row: usize, span_ms: std::ops::RangeInclusive<u64>) {
        self.put_format(records, hits, row, span_ms, crate::picture::deposit::count::CountFormat::Narrow);
    }

    fn put_format(
        &self,
        records: u64,
        hits: u64,
        row: usize,
        span_ms: std::ops::RangeInclusive<u64>,
        format: crate::picture::deposit::count::CountFormat,
    ) {
        let deposit = self.deposit(records, hits, row, span_ms, format);
        assert!(self.pipe.slot.put(deposit).is_ok());
    }

    fn deposit(
        &self,
        records: u64,
        hits: u64,
        row: usize,
        span_ms: std::ops::RangeInclusive<u64>,
        format: crate::picture::deposit::count::CountFormat,
    ) -> Deposit {
        let gen = oos_hal::sync::lock(&self.pipe.view).gen;
        let mut deposit = Deposit::new(gen, (4, self.picture.h), None, [true, false, false, false]);
        deposit.set_format(format);
        deposit.extent.include(self.origin + Duration::from_millis(*span_ms.start()));
        deposit.extent.include(self.origin + Duration::from_millis(*span_ms.end()));
        deposit.records = records;
        deposit.extent.add_hits(hits);
        deposit.edges[0] = records;
        // The pool can return bands in a different order, even with the same view.
        let rotation = row % deposit.planes.len();
        deposit.planes.rotate_left(rotation);
        for band in &mut deposit.planes {
            let mut left = hits;
            while left > 0 {
                let chunk = left.min(2730);
                band.test_span(row, (24 * chunk) as u16);
                left -= chunk;
            }
        }
        deposit
    }

    fn step(&mut self) -> Accepted {
        self.pipe.accept_into(
            &mut self.picture,
            &mut true,
            Stats { edges: &mut self.edges, off: &mut (0.0, 0.0) },
            Some(&mut self.window),
        )
    }

    fn rows(&self) -> [u16; 4] {
        std::array::from_fn(|row| {
            let pixels = &self.picture.maps()[0][row * 4..(row + 1) * 4];
            assert_eq!(pixels, [pixels[0]; 4]);
            pixels[0]
        })
    }
}

#[test]
fn newest_whole_deposits_fill_the_target_up_to_three() {
    let mut f = Fixture::new();
    f.put(27, 27, 0, 0..=0);
    assert_eq!(f.step().window_frames, Some(27));
    for row in 1..=3 {
        f.put(3, 3, row, row as u64..=row as u64);
        let accepted = f.step();
        assert_eq!(accepted.frames, 3);
        assert_eq!(accepted.window_frames, Some([0, 30, 33, 9][row]));
    }
    assert_eq!(f.rows(), [0, 72, 72, 72]);
    assert_eq!(f.edges[0], 36, "retained trigger statistics are counted once");
    assert!(f.pipe.slot.spare().is_some(), "eviction returns the set to the pool");
    f.put(27, 27, 0, 4..=4);
    assert_eq!(f.step().window_frames, Some(27));
    assert_eq!(f.rows(), [648, 0, 0, 0]);
}

#[test]
fn delayed_inputs_and_already_shown_records_have_no_age_cutoff() {
    let mut f = Fixture::new();
    f.put(9, 9, 0, 0..=40);
    let accepted = f.step();
    assert_eq!(accepted.newest, Some(f.origin + Duration::from_millis(40)));
    assert_eq!(accepted.window_frames, Some(9));
    assert_eq!(f.step().window_frames, None);
    assert_eq!(f.rows(), [216, 0, 0, 0]);

    let mut d = f.deposit(8, 8, 1, 201..=250, crate::picture::deposit::count::CountFormat::Narrow);
    let gen = oos_hal::sync::lock(&f.pipe.view).gen;
    let tap = crate::picture::deposit::Tap {
        frame: Arc::new(Frame { seq: 8, ..Frame::default() }),
        identity: None,
        projection: TapProjection::Projected { gen, size: (4, 4) },
        folds: None,
    };
    d.include_tap(f.pipe.slot.send_tap(tap, gen, None).unwrap());
    assert!(f.pipe.slot.put(d).is_ok());
    let accepted = f.step();
    let tap = f.pipe.take_waveform((4, 4)).unwrap();
    assert_eq!(tap.seq, 8);
    assert_eq!(tap.eligibility(), PictureEligibility::AlreadyDeposited);
    assert_eq!(accepted.frames, 8);
    assert_eq!(f.edges[0], 17);
    assert_eq!(f.pipe.counters.read().2, 0);
}

#[test]
fn dirty_publication_covers_disjoint_evicted_and_new_rows_then_clears_for_a_view_change() {
    let mut f = Fixture::new();
    f.put(1, 1, 1, 0..=0);
    f.step();
    f.picture.publish();
    assert_eq!(f.picture.published_rows(), 1..2);

    f.put(1, 1, 3, 1..=1);
    f.step();
    f.picture.publish();
    assert_eq!(f.picture.published_rows(), 1..4);
    assert_eq!(f.rows(), [0, 0, 0, 24]);

    f.step();
    f.picture.publish();
    assert!(f.picture.published_rows().is_empty());

    f.pipe.set_view(None, 4, 4, Interp::Exact);
    f.step();
    f.picture.publish();
    assert_eq!(f.picture.published_rows(), 3..4);
    assert_eq!(f.rows(), [0; 4]);

    f.step();
    f.picture.publish();
    assert!(f.picture.published_rows().is_empty());
}

#[test]
fn independent_uploads_cover_retained_replaced_and_invalidated_deposits() {
    let mut f = Fixture::new();
    let initial = f.picture.upload_rows(0, None).0;
    let mut stamps = [initial.clone(), initial];
    let mut maps = [vec![0; 16], vec![0; 16]];
    let steps = [(0, [648, 0, 0, 0]), (1, [648, 72, 0, 0]), (2, [648, 72, 72, 0]), (68, [0, 0, 0, 648]), (69, [0; 4])];
    for (step, (ms, expected)) in steps.into_iter().enumerate() {
        if step < 3 {
            let records = if step == 0 { 27 } else { 3 };
            f.put(records, records, step, ms..=ms);
        } else if step == 3 {
            f.put(27, 27, 3, ms..=ms);
        } else {
            f.pipe.set_view(None, 4, 4, Interp::Exact);
        }
        f.step();
        assert_eq!(f.rows(), expected);
        for consumer in 0..2 {
            if consumer == 1 && step % 2 == 0 && step < 4 {
                continue;
            }
            let (stamp, rows) = f.picture.upload_rows(0, Some(&stamps[consumer]));
            assert!(!rows.is_empty(), "consumer {consumer}, step {step}");
            let range = rows.start * 4..rows.end * 4;
            maps[consumer][range.clone()].copy_from_slice(&f.picture.maps()[0][range]);
            stamps[consumer] = stamp;
            assert_eq!(maps[consumer], f.picture.maps()[0], "consumer {consumer}, step {step}");
        }
    }
    assert_eq!(maps, [vec![0; 16], vec![0; 16]]);
    assert_eq!(f.edges[0], 60, "each record is counted once");
    assert_eq!(f.pipe.counters.read().2, 0);
}

#[test]
fn combined_capacity_evicts_the_oldest_whole_deposit() {
    let mut f = Fixture::new();
    f.put(27, 27, 0, 0..=0);
    f.step();
    // Two dense dot deposits fit exactly on their own; all three exceed the limit.
    f.put(3, 1365, 1, 1..=1);
    assert_eq!(f.step().window_frames, Some(30));
    f.put(3, 1365, 1, 2..=2);
    assert_eq!(f.step().window_frames, Some(6));
    assert_eq!(f.rows(), [0, 65_520, 0, 0]);
    f.put(3, 1366, 1, 3..=3);
    assert_eq!(f.step().window_frames, Some(3));
    assert_eq!(f.rows(), [0, 32_784, 0, 0]);
}

#[test]
fn wide_combined_window_uses_its_own_limit_and_recycles_without_mixing_formats() {
    use crate::picture::deposit::count::CountFormat;
    let mut f = Fixture::new();
    let put = |f: &Fixture, records, hits, row, ms| f.put_format(records, hits, row, ms..=ms, CountFormat::Wide);
    put(&f, 27, 2884, 0, 0);
    assert_eq!(f.step().window_frames, Some(27));
    assert_eq!(f.picture.count(0, 0), 69_216);
    let half = CountFormat::Wide.hit_limit() / 2;
    put(&f, 3, half, 1, 1);
    assert_eq!(f.step().window_frames, Some(30));
    put(&f, 3, half, 1, 2);
    assert_eq!(f.step().window_frames, Some(6));
    assert_eq!(f.picture.count(0, 4), (2 * half * 24) as u32);
    put(&f, 3, half + 2, 1, 3);
    assert_eq!(f.step().window_frames, Some(3));
    assert_eq!(f.picture.count(0, 4), ((half + 2) * 24) as u32);
    f.put(3, 1, 2, 4..=4);
    assert_eq!(f.step().window_frames, Some(3));
    assert!(!f.picture.is_wide());
    assert_eq!(f.rows(), [0, 0, 24, 0]);
    assert!(f.pipe.slot.spare().is_some());
}

#[test]
fn retained_narrow_bands_publish_the_fold_hull_and_clear_the_gap() {
    let mut f = Fixture::with_height(16);
    let initial = f.picture.upload_rows(0, None).0;
    let mut panel = initial.clone();
    let mut uploaded = vec![0; 64];
    for (records, row, ms, expected) in [(27, 2, 0, 2..3), (3, 12, 1, 2..13), (27, 8, 2, 2..13)] {
        f.put(records, records, row, ms..=ms);
        f.step();
        assert!(!f.picture.is_wide());
        let (next, rows) = f.picture.upload_rows(0, Some(&panel));
        assert_eq!(rows, expected);
        let pixels = rows.start * 4..rows.end * 4;
        uploaded[pixels.clone()].copy_from_slice(&f.picture.maps()[0][pixels]);
        assert_eq!(uploaded, f.picture.maps()[0]);
        panel = next;
        if ms == 1 {
            assert!(uploaded[8..12].iter().all(|&v| v == 27 * 24));
            assert!(uploaded[48..52].iter().all(|&v| v == 3 * 24));
            assert!(uploaded[12..48].iter().all(|&v| v == 0), "disjoint active bands keep their gap");
        }
    }
    assert_eq!(f.picture.upload_rows(0, Some(&initial)).1, 2..13);
    assert!(f.picture.maps()[0][8..32].iter().all(|&v| v == 0), "evicted rows and the gap clear");
    assert!(f.picture.maps()[0][32..36].iter().all(|&v| v == 27 * 24));
    assert!(f.picture.maps()[0][36..].iter().all(|&v| v == 0));
    assert_eq!(f.step().window_frames, None);
    assert!(f.picture.upload_rows(0, Some(&panel)).1.is_empty());
    f.pipe.set_view(None, 4, 16, Interp::Exact);
    assert_eq!(f.step().window_frames, Some(0));
    assert_eq!(f.picture.upload_rows(0, Some(&panel)).1, 8..9);
    assert!(f.picture.maps()[0].iter().all(|&v| v == 0));
}

// A partial batch cannot reach the picture while a decoder is blocked. A completed merge reports its read time.
#[test]
fn partial_merges_and_picture_age_are_counted() {
    // This legacy pacing fixture writes process-global mirrors. Give only this test
    // a child process so concurrent UI publication cannot replace its WANTED value.
    const CHILD: &str = "OOS_PARTIAL_MERGE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "drawpipe::tests::window::partial_merges_and_picture_age_are_counted", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let m = model(1e-6, false, false);
    let mut ph = Phosphor::new(970, 448);
    let mut merge = |pipe: &Pipe| pipe.accept(&mut ph, &mut true, &mut [0; 4], &mut (0.0, 0.0));
    // Inject the read timestamp on an identified submission, then keep the legacy stamp fixture below.
    let timed = Pipe::start(2, 0, fresh());
    let id = oos_core::change::Identity::default();
    timed.publish_view(&m, 970, 448, Interp::Exact, id);
    let (bytes, fb, s, p) = batch(20, 12_512, false);
    let read_at = Instant::now();
    timed.feed_at(Box::new(Bytes(bytes)), fb, 0..20, (id, read_at), dec(&s, &p), None, None);
    settle(&timed);
    let whole = merge(&timed);
    assert_eq!(whole.frames, 20);
    assert_eq!(whole.newest, Some(read_at), "picture age starts at the read timestamp");
    assert_eq!(crate::pacing::PARTIAL.load(Ordering::Relaxed), 0);
    let pipe = Pipe::start(2, 0, fresh());
    pipe.set_view(Some(&m), 970, 448, Interp::Exact);
    // The whole batch becomes available once the held worker can finish.
    let gate = super::worker_shutdown::one_worker_held(&pipe);
    let first = merge(&pipe).frames;
    *gate.0.lock().unwrap() = true;
    gate.1.notify_all();
    settle(&pipe);
    let second = merge(&pipe).frames;
    assert_eq!((first, second), (0, 20));
    assert_eq!(crate::pacing::PARTIAL.load(Ordering::Relaxed), 0);
    // an empty merge reports no age
    assert_eq!(merge(&pipe).newest, None);
    // stale: frames acquired before a settings change count where their maps reach the picture, not where they
    // are drawn: maps of an older mapping are dropped and do not count
    crate::pacing::changed();
    crate::pacing::changed();
    let wanted = crate::pacing::WANTED.load(Ordering::Relaxed);
    crate::pacing::APPLIED.store(wanted - 1, Ordering::Relaxed);
    let (bytes, fb, s, p) = batch(20, 12_512, false);
    pipe.feed(Box::new(Bytes(bytes)), fb, 20, 0, dec(&s, &p));
    crate::pacing::APPLIED.store(0, Ordering::Relaxed);
    settle(&pipe);
    assert_eq!(merge(&pipe).stale, 20, "shown");
    let (bytes, fb, s, p) = batch(20, 12_512, false);
    crate::pacing::APPLIED.store(wanted - 1, Ordering::Relaxed);
    pipe.feed(Box::new(Bytes(bytes)), fb, 20, 0, dec(&s, &p));
    crate::pacing::APPLIED.store(0, Ordering::Relaxed);
    settle(&pipe);
    let mut m2 = m.clone();
    m2.tscale += 1;
    pipe.set_view(Some(&m2), 970, 448, Interp::Exact);
    assert_eq!(merge(&pipe).stale, 0, "maps of the old mapping dropped, not counted");
}
