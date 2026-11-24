// SPDX-License-Identifier: GPL-2.0-or-later

//! Screen-line geometry shared by all records under a zoom projection.

use crate::model::{HDIV, NCH, VDIV};
use crate::picture::deposit::count::Count;
use std::ops::Range;

mod polyphase;
pub(crate) use polyphase::{Filter, Window};

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct Geometry {
    width: usize,
    dt: f64,
    time_per_div: f64,
    delay: f64,
    origin: f64,
    points: usize,
}

impl Geometry {
    pub(crate) fn new(width: usize, (dt, time_per_div, delay, origin): (f64, f64, f64, f64), points: usize) -> Self {
        Self { width, dt, time_per_div, delay, origin, points }
    }

    fn x(self, k: usize) -> f32 {
        let width = self.width as f32 as f64;
        (((self.origin + k as f64 * self.dt - self.delay) / self.time_per_div) * (width / f64::from(HDIV))
            + width / 2.0) as f32
    }

    fn supported(self) -> bool {
        self.points >= 2
            && self.dt > 0.0
            && self.time_per_div > 0.0
            && self.x(0).is_finite()
            && self.x(self.points - 1).is_finite()
    }
}

/// The first vertex at or beyond this grid line, and the intersection of the segment that crosses it.
struct Boundary {
    vertex: usize,
    fraction: f32,
}

#[derive(Default)]
struct Grid {
    geometry: Option<Geometry>,
    boundaries: Vec<Boundary>,
}

impl Grid {
    fn prepare(&mut self, grid: Geometry) {
        if self.geometry == Some(grid) {
            return;
        }
        self.geometry = Some(grid);
        self.boundaries.clear();
        let mut vertex = 0;
        let (mut previous, mut next) = (0.0, grid.x(0));
        self.boundaries.extend((0..=grid.width).map(|column| {
            let column = column as f32;
            while vertex < grid.points && next < column {
                vertex += 1;
                previous = next;
                if vertex < grid.points {
                    next = grid.x(vertex);
                }
            }
            let fraction = if 0 < vertex && vertex < grid.points {
                ((column - previous) / (next - previous)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            Boundary { vertex, fraction }
        }));
    }
}

#[inline]
fn crossing_rows(samples: &[f32], vertex: usize, (first, last): (f32, f32)) -> Option<(i32, i32)> {
    let (a, b) = (samples[vertex - 1], samples[vertex]);
    if a.is_nan() || b.is_nan() {
        return None;
    }
    let dy = b - a;
    let (a, b) = (a + dy * first, a + dy * last);
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    Some((lo.round() as i32, hi.round() as i32))
}

#[derive(Default)]
pub(crate) struct Scratch {
    grids: [[Grid; 2]; NCH],
    projected: Vec<f32>,
}

pub(crate) fn paint<T: Count>(
    samples: &[f32],
    geometry: Geometry,
    target: (&mut [T], usize, usize),
    vertical: (f32, f32),
    channel: usize,
) -> Option<Range<usize>> {
    if samples.len() < 2 {
        return Some(0..0);
    }
    if !geometry.supported() || !vertical.0.is_finite() || vertical.0 <= 0.0 || !vertical.1.is_finite() {
        return None;
    }
    crate::fastdraw::with_scratch(|scratch| {
        let scratch = &mut scratch.routing.zoom.lines;
        let grids = &mut scratch.grids[channel];
        if grids[0].geometry != Some(geometry) {
            grids.swap(0, 1);
        }
        let grid = &mut grids[0];
        grid.prepare(geometry);
        let rows = &mut scratch.projected;
        project_rows(samples, target.1, vertical, rows);
        Some(paint_columns(rows, target, &grid.boundaries))
    })
}

fn project_rows(samples: &[f32], height: usize, (volts_per_div, offset): (f32, f32), out: &mut Vec<f32>) {
    let ht = height as f32;
    let cy = ht / 2.0;
    let ppd = ht / VDIV;
    out.clear();
    out.extend(samples.iter().map(|&v| if v.is_finite() { cy - (v + offset) / volts_per_div * ppd } else { f32::NAN }));
}

fn paint_columns<T: Count>(
    samples: &[f32],
    (band, height, first): (&mut [T], usize, usize),
    boundaries: &[Boundary],
) -> Range<usize> {
    let width = band.len() / (height + 1);
    let (mut first_row, mut last_row) = (height, 0);
    for (x, edges) in boundaries[first..=first + width].windows(2).enumerate() {
        let mut write = |(lo, hi): (i32, i32), weight| {
            let (lo, hi) = (lo.max(0), hi.min(height as i32 - 1));
            if lo <= hi {
                super::diff_span(band, width, x, lo as usize, hi as usize, weight);
                first_row = first_row.min(lo as usize);
                last_row = last_row.max(hi as usize + 1);
            }
        };
        let (left, right) = (&edges[0], &edges[1]);
        let start = left.vertex.saturating_sub(1);
        let end = (right.vertex + 1).min(samples.len());
        let points = &samples[start..end];
        if points.len() > 1 && points.len() <= usize::from(u16::MAX / 24) {
            if let Some(row) = flat_row(points) {
                write((row, row), (points.len() - 1) as u16 * 24);
                continue;
            }
        }
        trace_column(samples, left, right, |rows| write(rows, 24));
    }
    first_row.min(last_row)..last_row
}

#[inline]
fn flat_row(points: &[f32]) -> Option<i32> {
    let (&first, rest) = points.split_first()?;
    let row = first.round();
    // Inside these limits, rounding error in a + (b - a) * t is less than the margin at a pixel edge.
    let (lo, hi) = (row - 0.499, row + 0.499);
    (row.abs() <= 4096.0 && first >= lo && first <= hi && rest.iter().all(|&y| lo <= y && y <= hi))
        .then_some(row as i32)
}

#[inline]
fn trace_column(samples: &[f32], left: &Boundary, right: &Boundary, mut span: impl FnMut((i32, i32))) {
    let (first, end) = (left.vertex, right.vertex);
    if 0 < first && first < samples.len() {
        let last = if first == end { right.fraction } else { 1.0 };
        if let Some(rows) = crossing_rows(samples, first, (left.fraction, last)) {
            span(rows);
        }
    }
    let mut previous = None;
    for &value in &samples[first..end] {
        let row = (!value.is_nan()).then(|| value.round() as i32);
        if let (Some(a), Some(b)) = (previous, row) {
            span((i32::min(a, b), i32::max(a, b)));
        }
        previous = row;
    }
    if first < end && end < samples.len() {
        if let Some(rows) = crossing_rows(samples, end, (0.0, right.fraction)) {
            span(rows);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bands_keep_each_legacy_segment_and_its_exact_weight() {
        let samples = [f32::NAN, -4.0, -1.0, -0.1, 0.22, 0.3, 0.3, f32::INFINITY, 8.0, 0.5, 0.5, 0.5, 0.5, -0.8];
        let vertical = (0.31, -0.19);
        let height = 15;
        let width = 17;
        let (div, delay) = (7.0, -1.0);
        for dt in [0.73, 4.5, 12.25] {
            for origin in [-59.9, -40.5, -32.3, -6.0] {
                let geometry = Geometry::new(width, (dt, div, delay, origin), samples.len());
                for (first, last) in [(0, 5), (5, 8), (8, 17)] {
                    let bw = last - first;
                    let mut reference = vec![0u16; bw * (height + 1)];
                    crate::wave::trace_columns(
                        width,
                        height,
                        vertical,
                        false,
                        (div, delay),
                        origin,
                        dt,
                        &samples,
                        24,
                        None,
                        false,
                        (first as i32, last as i32),
                        |x, lo, hi, weight| {
                            super::super::diff_span(
                                &mut reference,
                                bw,
                                x as usize - first,
                                lo as usize,
                                hi as usize,
                                weight,
                            );
                        },
                    );
                    let mut actual = vec![0u16; reference.len()];
                    paint(&samples, geometry, (&mut actual, height, first), vertical, 0).unwrap();
                    actual.fill(0);
                    let (allocations, rows) = crate::system::test_allocator::count_own(|| {
                        paint(&samples, geometry, (&mut actual, height, first), vertical, 0).unwrap()
                    });
                    assert_eq!(allocations, 0);
                    assert_eq!(actual, reference, "origin {origin}, dt {dt}, columns {first}..{last}");
                    for (row, cells) in actual.chunks_exact(bw).enumerate() {
                        if cells.iter().any(|&cell| cell != 0) {
                            assert!(rows.start <= row && row <= rows.end);
                        }
                    }
                }
            }
        }
    }
}
