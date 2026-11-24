// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

fn check(edges: &mut ColumnEdges, width: usize, len: usize, map: &Map) {
    edges.update(width, len, map);
    let columns = fastdraw::columns(width, len, map);
    if columns.is_empty() {
        assert!(edges.ranges().first.is_empty(), "{map:?}");
    } else {
        let expected: Vec<_> =
            (columns.start..=columns.end).map(|column| fastdraw::first_sample(column, len, map)).collect();
        assert_eq!(edges.ranges().c0, columns.start as usize, "{map:?}");
        assert_eq!(edges.ranges().first, expected, "{map:?}");
    }

    edges.update_for(width, len, map, true);
    let first = (centre(0, map).floor() as usize).min(width);
    let end = (centre(len.div_ceil(2) - 1, map).floor() as i64).saturating_add(1).clamp(0, width as i64) as usize;
    if first >= end {
        assert!(edges.ranges().first.is_empty(), "{map:?}");
        return;
    }
    assert_eq!(edges.ranges().c0, first, "{map:?}");
    for (offset, column) in (first..=end).enumerate() {
        let expected = legacy_pair_edge(column, len, map);
        assert_eq!(
            (edges.ranges().first[offset], edges.ranges().previous_pair[offset]),
            expected,
            "{map:?}, column={column}"
        );
    }
}

#[test]
fn alternating_projections_reuse_grids_including_rejections() {
    let mut edges = ColumnEdges::default();
    let overview = Map { ti: 0.0, x_per_sample: 0.5, x0: 10.0, y0: 0.0, y_per_volt: 1.0 };
    let detail = Map { x_per_sample: 1e-16, ..overview };
    check(&mut edges, 97, 384, &overview);
    check(&mut edges, 97, 384, &detail);
    assert_eq!(edges.grid_rebuilds(), 4);
    assert!(edges.grids[edges.active.unwrap()].forward.is_empty());
    for ti in [1.0, 1.25, 2.0] {
        check(&mut edges, 97, 384, &Map { ti, ..overview });
        check(&mut edges, 97, 384, &Map { ti, ..detail });
    }
    assert_eq!(edges.grid_rebuilds(), 4);
}

#[test]
fn exact_edges_at_binary64_boundaries() {
    let mut edges = ColumnEdges::default();
    let mut rng = crate::wave::Rng::new(99);
    for case in 0..1200 {
        let len = [1, 17, 970, 14065, 10_000_000][case % 5];
        let x_per_sample = [0.001, 0.0625, 0.17, 0.5, 2.0, 1024.0, 1e-9, rng.uniform().mul_add(0.9, 0.00001)][case % 8];
        let width = 97;
        let step = x_per_sample.recip();
        let x0 = rng.uniform().mul_add(2.0 * width as f64, -(width as f64) / 2.0);
        let column = (rng.next() as usize % width) as f64;
        let inverse = (column - x0) * step;
        let centre = rng.next() as usize % len;
        let boundary = centre as f64 - inverse;
        let forward_boundary = (centre | 1) as f64 - (column - x0) / x_per_sample;
        for ti in [
            0.0,
            0.0f64.next_up(),
            boundary.next_down(),
            boundary,
            boundary.next_up(),
            forward_boundary.next_down(),
            forward_boundary,
            forward_boundary.next_up(),
        ] {
            check(&mut edges, width, len, &Map { ti, x_per_sample, x0, y0: 0.0, y_per_volt: 1.0 });
        }
    }
}

#[test]
fn fixed_geometry_and_phase_reuse_exact_edges_until_the_length_changes() {
    let mut edges = ColumnEdges::default();
    let main = Map { ti: 45.375, x_per_sample: 0.5, x0: 48.0, y0: 0.0, y_per_volt: 1.0 };
    let zoom = Map { x_per_sample: 1.5, ..main };
    edges.update(97, 384, &main);
    let overview = edges.ranges().first.clone();
    edges.update(97, 384, &zoom);
    let detail = edges.ranges().first.clone();
    let (allocations, ()) = crate::system::test_allocator::count_own(|| {
        edges.update(97, 384, &main);
        assert_eq!(edges.ranges().first, overview);
        edges.update(97, 384, &zoom);
        assert_eq!(edges.ranges().first, detail);
    });
    assert_eq!(allocations, 0);
    assert_eq!(edges.rebuilds, (2, 2));

    edges.update(97, 67, &main);
    assert_eq!(edges.ranges().first.last(), Some(&67));
    assert_eq!(edges.rebuilds, (2, 3));
}

#[test]
fn dense_edges_preserve_binary64_addition_at_all_exponents() {
    let mut rng = crate::wave::Rng::new(410);
    let mut actual = Vec::new();
    for exp in 0..=0x7ffu64 {
        let a = f64::from_bits(exp << 52 | (rng.next() & MANTISSA));
        for phase in [a, -a, a.next_down(), -a.next_up()] {
            let inverse = [1.0, a, -a, -a.next_down(), -a.next_up()];
            let len = 10_000_000;
            let expected = inverse.map(|x| ((phase + x).ceil().max(0.0) as usize).min(len));
            actual.clear();
            dense_edges(&inverse, phase, len, &mut actual);
            assert_eq!(actual, expected, "{phase:?} + {inverse:?}");
        }
    }
}
