// SPDX-License-Identifier: GPL-2.0-or-later

use super::Picture;

#[derive(Clone, Copy, Default)]
struct Column {
    bounds: Option<(usize, usize)>,
    pixels: usize,
}

impl Column {
    fn holes(self) -> usize {
        self.bounds.map_or(0, |(min, max)| max - min + 1 - self.pixels)
    }
}

struct Coverage {
    columns: Vec<Column>,
    lit_columns: usize,
    lit_pixels: usize,
    longest_gap: usize,
}

impl Coverage {
    fn of(map: &[u16], width: usize) -> Self {
        let mut columns = vec![Column::default(); width];
        for (row, counts) in map.chunks_exact(width).enumerate() {
            for (col, &count) in columns.iter_mut().zip(counts) {
                if count != 0 {
                    let min = col.bounds.map_or(row, |(min, _)| min);
                    col.bounds = Some((min, row));
                    col.pixels += 1;
                }
            }
        }
        let occupied = |col: &Column| col.bounds.is_some();
        let lit_columns = columns.iter().filter(|col| occupied(col)).count();
        let lit_pixels = columns.iter().map(|col| col.pixels).sum();
        let longest_gap =
            columns.iter().position(occupied).zip(columns.iter().rposition(occupied)).map_or(0, |(first, last)| {
                columns[first..=last].split(|col| occupied(col)).map(<[_]>::len).max().unwrap_or(0)
            });
        Self { columns, lit_columns, lit_pixels, longest_gap }
    }

    fn assert_matches(&self, records: &Self, enabled: bool, label: &str) {
        assert_eq!(self.lit_columns, records.lit_columns, "{label}: lit columns");
        assert_eq!(self.longest_gap, records.longest_gap, "{label}: longest interior gap");
        assert_eq!(records.lit_pixels > 0, enabled, "{label}: Records channel occupancy");
        let mut pixel_tolerance = 0;
        for (x, (&batch, &reference)) in self.columns.iter().zip(&records.columns).enumerate() {
            match (batch.bounds, reference.bounds) {
                (Some((lo, hi)), Some((ref_lo, ref_hi))) => {
                    let (lo_delta, hi_delta) = (lo.abs_diff(ref_lo), hi.abs_diff(ref_hi));
                    // The fixed-point interpolation can round an endpoint by one row. The full-width
                    // fixture has no clipped time edges, so column occupancy and interior gaps must match.
                    assert!(
                        lo_delta <= 1 && hi_delta <= 1,
                        "{label}: column {x} rows {lo}..={hi} vs {ref_lo}..={ref_hi}"
                    );
                    assert_eq!(batch.holes(), reference.holes(), "{label}: column {x} interior pixel gaps");
                    pixel_tolerance += lo_delta + hi_delta;
                }
                (batch, reference) => assert_eq!(batch, reference, "{label}: column {x} occupancy"),
            }
        }
        // Only pixels at the rounded endpoints may change the lit-pixel count.
        assert!(
            self.lit_pixels.abs_diff(records.lit_pixels) <= pixel_tolerance,
            "{label}: lit pixels {} vs {}, tolerance {pixel_tolerance}",
            self.lit_pixels,
            records.lit_pixels
        );
    }
}

pub(super) fn compare(records: &Picture, batch: &Picture, width: usize, channels: usize, label: &str) {
    assert_eq!(batch.frames, records.frames, "{label}: counted frames");
    assert!(batch.zoom == records.zoom, "{label}: zoom pixel maps");
    for (c, (records, batch)) in records.main.iter().zip(&batch.main).enumerate() {
        Coverage::of(batch, width).assert_matches(
            &Coverage::of(records, width),
            c < channels,
            &format!("{label} CH{}", c + 1),
        );
    }
}
