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
        let gen = oos_hal::sync::lock(&self.pipe.view).gen;
        let mut deposit = Deposit::new(gen, (4, 4), None, [true, false, false, false]);
        deposit.extent.include(self.origin + Duration::from_millis(*span_ms.start()));
        deposit.extent.include(self.origin + Duration::from_millis(*span_ms.end()));
        deposit.records = records;
        deposit.extent.add_hits(hits);
        deposit.edges[0] = records;
        deposit.newest = Some(Arc::new(Frame { seq: records, ..Frame::default() }));
        // The pool can return bands in a different order, even with the same view.
        deposit.planes.rotate_left(row);
        for band in &mut deposit.planes {
            crate::fastdraw::diff_span(&mut band.plane.d, 1, 0, row, row, (24 * hits) as u16);
            band.touch(row..row + 1);
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
            let pixels = &self.picture.map[0][row * 4..(row + 1) * 4];
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
    let rejected = f.step(138);
    assert_eq!(rejected.frames, 0);
    assert!(rejected.age_rejected);
    assert_eq!(rejected.waveform.unwrap().seq, 8);
    assert_eq!(f.edges[0], 9);
    assert!(f.pipe.counters.records_text().ends_with("rejected.age=8"));
    assert_eq!(f.pipe.counters.read().2, 8);
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
