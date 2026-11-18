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
        let pipe = Pipe::start(0, 0, fresh());
        pipe.set_view(Some(&Model::default()), 4, 4, Interp::Exact);
        Self { pipe, window: Default::default(), picture: Phosphor::new(4, 4), origin: Instant::now(), edges: [0; 4] }
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
        let gen = oos_hal::sync::lock(&self.pipe.view).gen;
        let mut deposit = Deposit::new(gen, (4, 4), None, [true, false, false, false]);
        deposit.set_format(format);
        deposit.extent.include(self.origin + Duration::from_millis(*span_ms.start()));
        deposit.extent.include(self.origin + Duration::from_millis(*span_ms.end()));
        deposit.records = records;
        deposit.extent.add_hits(hits);
        deposit.edges[0] = records;
        // The pool can return bands in a different order, even with the same view.
        deposit.planes.rotate_left(row);
        for band in &mut deposit.planes {
            let mut left = hits;
            while left > 0 {
                let chunk = left.min(2730);
                band.test_span(row, (24 * chunk) as u16);
                left -= chunk;
            }
        }
        assert!(self.pipe.slot.put(deposit).is_ok());
    }

    fn step(&mut self, ms: u64) -> Accepted {
        self.pipe.accept_into(
            &mut self.picture,
            &mut true,
            Stats { edges: &mut self.edges, off: &mut (0.0, 0.0) },
            Some((&mut self.window, self.origin + Duration::from_millis(ms))),
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
    assert_eq!(f.step(0).window_frames, Some(27));
    for row in 1..=3 {
        f.put(3, 3, row, row as u64..=row as u64);
        let accepted = f.step(row as u64);
        assert_eq!(accepted.frames, 3);
        assert_eq!(accepted.window_frames, Some([0, 30, 33, 9][row]));
    }
    assert_eq!(f.rows(), [0, 72, 72, 72]);
    assert_eq!(f.edges[0], 36, "retained trigger statistics are counted once");
    assert!(f.pipe.slot.spare().is_some(), "eviction returns the set to the pool");
    f.put(27, 27, 0, 4..=4);
    assert_eq!(f.step(4).window_frames, Some(27));
    assert_eq!(f.rows(), [648, 0, 0, 0]);
}

#[test]
fn oldest_read_bounds_age_at_acceptance_and_without_new_input() {
    let mut f = Fixture::new();
    f.put(9, 9, 0, 0..=40);
    let accepted = f.step(67);
    assert_eq!(accepted.newest, Some(f.origin + Duration::from_millis(40)));
    assert_eq!(accepted.window_frames, Some(9));
    assert_eq!(f.step(67).window_frames, None);
    assert_eq!(f.step(68).window_frames, Some(0));
    assert_eq!(f.rows(), [0; 4]);

    f.put(8, 8, 1, 70..=138);
    let gen = oos_hal::sync::lock(&f.pipe.view).gen;
    let tap = crate::picture::deposit::Tap {
        frame: Arc::new(Frame { seq: 8, ..Frame::default() }),
        identity: None,
        projection: TapProjection::Projected { gen, size: (4, 4) },
        folds: None,
    };
    assert!(f.pipe.slot.send_tap(tap, gen, None).is_some());
    let rejected = f.step(138);
    assert_eq!(rejected.frames, 0);
    let tap = f.pipe.take_waveform((4, 4)).unwrap();
    assert_eq!(tap.seq, 8);
    assert_eq!(tap.eligibility(), PictureEligibility::Pending);
    assert_eq!(f.edges[0], 9);
    assert!(f.pipe.counters.records_text().ends_with("rejected.age=8"));
    assert_eq!(f.pipe.counters.read().2, 8);
}

#[test]
fn dirty_publication_covers_disjoint_evicted_and_new_rows_then_expires() {
    let mut f = Fixture::new();
    f.put(1, 1, 1, 0..=0);
    f.step(0);
    f.picture.publish();
    assert_eq!(f.picture.published_rows(), 1..2);

    f.put(1, 1, 3, 1..=1);
    f.step(1);
    f.picture.publish();
    assert_eq!(f.picture.published_rows(), 1..4);
    assert_eq!(f.rows(), [0, 0, 0, 24]);

    f.step(2);
    f.picture.publish();
    assert!(f.picture.published_rows().is_empty());

    f.step(69);
    f.picture.publish();
    assert_eq!(f.picture.published_rows(), 3..4);
    assert_eq!(f.rows(), [0; 4]);

    f.step(70);
    f.picture.publish();
    assert!(f.picture.published_rows().is_empty());
}

#[test]
fn independent_uploads_include_retained_and_expired_deposits() {
    let mut f = Fixture::new();
    let initial = f.picture.upload_rows(0, None).0;
    let mut stamps = [initial.clone(), initial];
    let mut maps = [vec![0; 16], vec![0; 16]];
    let steps = [
        (0, [648, 0, 0, 0]),
        (1, [648, 72, 0, 0]),
        (2, [648, 72, 72, 0]),
        (68, [0, 72, 72, 0]),
        (69, [0, 0, 72, 0]),
        (70, [0; 4]),
    ];
    for (step, (ms, expected)) in steps.into_iter().enumerate() {
        if step < 3 {
            let records = if step == 0 { 27 } else { 3 };
            f.put(records, records, step, ms..=ms);
        }
        f.step(ms);
        assert_eq!(f.rows(), expected);
        for consumer in 0..2 {
            if consumer == 1 && step % 2 == 0 {
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
    assert_eq!(f.edges[0], 33, "retained and expired records are counted once");
    assert_eq!(f.pipe.counters.read().2, 0);
}

#[test]
fn combined_capacity_evicts_the_oldest_whole_deposit() {
    let mut f = Fixture::new();
    f.put(27, 27, 0, 0..=0);
    f.step(0);
    // Two dense dot deposits fit exactly on their own; all three exceed the limit.
    f.put(3, 1365, 1, 1..=1);
    assert_eq!(f.step(1).window_frames, Some(30));
    f.put(3, 1365, 1, 2..=2);
    assert_eq!(f.step(2).window_frames, Some(6));
    assert_eq!(f.rows(), [0, 65_520, 0, 0]);
    f.put(3, 1366, 1, 3..=3);
    assert_eq!(f.step(3).window_frames, Some(3));
    assert_eq!(f.rows(), [0, 32_784, 0, 0]);
}

#[test]
fn wide_combined_window_uses_its_own_limit_and_recycles_without_mixing_formats() {
    use crate::picture::deposit::count::CountFormat;
    let mut f = Fixture::new();
    let put = |f: &Fixture, records, hits, row, ms| f.put_format(records, hits, row, ms..=ms, CountFormat::Wide);
    put(&f, 27, 2884, 0, 0);
    assert_eq!(f.step(0).window_frames, Some(27));
    assert_eq!(f.picture.count(0, 0), 69_216);
    let half = CountFormat::Wide.hit_limit() / 2;
    put(&f, 3, half, 1, 1);
    assert_eq!(f.step(1).window_frames, Some(30));
    put(&f, 3, half, 1, 2);
    assert_eq!(f.step(2).window_frames, Some(6));
    assert_eq!(f.picture.count(0, 4), (2 * half * 24) as u32);
    put(&f, 3, half + 2, 1, 3);
    assert_eq!(f.step(3).window_frames, Some(3));
    assert_eq!(f.picture.count(0, 4), ((half + 2) * 24) as u32);
    f.put(3, 1, 2, 4..=4);
    assert_eq!(f.step(4).window_frames, Some(3));
    assert!(!f.picture.is_wide());
    assert_eq!(f.rows(), [0, 0, 24, 0]);
    assert!(f.pipe.slot.spare().is_some());
}
