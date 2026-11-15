// SPDX-License-Identifier: GPL-2.0-or-later

use super::oracle::{self, Row};
use oos_core::change::{Classes, FieldDiff, FIELD_COUNT};
use std::fmt::Write;

pub(in crate::ui) struct Diagnostics {
    pub(super) raw: [u64; 10],
    pub(super) normalized: u64,
    pub(super) outbox: u64,
    pub(super) errors: u64,
    pub(super) unreviewed: u64,
    error_counts: [u64; 5],
    first: [bool; FIELD_COUNT],
    event_first: [u16; oracle::ROW_COUNT],
}
impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            raw: [0; 10],
            normalized: 0,
            outbox: 0,
            errors: 0,
            unreviewed: 0,
            error_counts: [0; 5],
            first: [false; FIELD_COUNT],
            event_first: [0; oracle::ROW_COUNT],
        }
    }
}
impl Diagnostics {
    pub(super) fn error(&mut self, error: oos_core::change::Error) {
        use oos_core::change::Error;
        let i = match error {
            Error::Full => 0,
            Error::PayloadTooLarge => 1,
            Error::SnapshotTooLarge => 2,
            Error::DuplicateToken => 3,
            Error::GenerationOverflow => 4,
        };
        self.errors += 1;
        self.error_counts[i] += 1;
    }
    pub(super) fn compare(
        &mut self,
        new: Classes,
        old: Classes,
        rows: u32,
        by_row: &[Classes; oracle::ROW_COUNT],
        stopped: bool,
        repaired: Classes,
    ) -> u16 {
        let mut differences = 0;
        for (i, c) in Classes::ALL.into_iter().enumerate() {
            if new.contains(c) == old.contains(c) {
                continue;
            }
            self.raw[i] += 1;
            let found = Row::ALL
                .into_iter()
                .filter(|r| rows & (1 << *r as usize) != 0 && (!old.contains(c) || by_row[*r as usize].contains(c)))
                .find_map(|r| oracle::difference(r, c, old.contains(c), stopped, repaired).map(|d| (r, d)));
            let row = found.map_or(Row::Drawing, |(row, _)| row);
            let difference = found.map_or("unreviewed", |(_, d)| d.name());
            if let Some((_, difference)) = found {
                differences |= difference.mask();
            } else {
                self.unreviewed += 1;
                differences |= 1 << 15;
            }
            if self.event_first[row as usize] & (1 << i) == 0 {
                eprintln!(
                    "oos-ui shadow: row={} class={} old={} new={} difference={}",
                    row.name(),
                    Classes::NAMES[i],
                    old.contains(c),
                    new.contains(c),
                    difference
                );
                self.event_first[row as usize] |= 1 << i;
            }
        }
        differences
    }
    pub(super) fn field(&mut self, f: FieldDiff, row: Row, differences: u16) {
        if f.changed() && !self.first[f.index()] {
            self.first[f.index()] = true;
            // The fixed mask is D1..D9 in bits 0..8; bit 15 is unreviewed. Class logs spell out each id.
            eprintln!(
                "oos-ui shadow: first field={} row={} classes={:?} difference_ids=0x{:04x}",
                f.name(),
                row.name(),
                f.classes(),
                differences
            );
        }
    }
    pub(super) fn text(&self) -> String {
        let mut s = String::from(" shadow");
        for (name, count) in Classes::NAMES.into_iter().zip(self.raw) {
            let _ = write!(s, " {name}={count}");
        }
        let _ = write!(
            s,
            " normalize={} outbox={} errors={} unreviewed={}",
            self.normalized, self.outbox, self.errors, self.unreviewed
        );
        for (name, count) in ["Full", "PayloadTooLarge", "SnapshotTooLarge", "DuplicateToken", "GenerationOverflow"]
            .into_iter()
            .zip(self.error_counts)
        {
            let _ = write!(s, " {name}={count}");
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oos_core::change::Error;

    #[test]
    fn counts_each_error_variant_and_the_total() {
        let mut d = Diagnostics::default();
        for error in [
            Error::Full,
            Error::PayloadTooLarge,
            Error::SnapshotTooLarge,
            Error::DuplicateToken,
            Error::GenerationOverflow,
        ] {
            d.error(error);
        }
        d.error(Error::DuplicateToken);
        assert_eq!(d.errors, 6);
        let text = d.text();
        for expected in [
            "Full=1",
            "PayloadTooLarge=1",
            "SnapshotTooLarge=1",
            "DuplicateToken=2",
            "GenerationOverflow=1",
            "errors=6",
        ] {
            assert!(text.contains(expected), "{text}");
        }
    }
}

#[cfg(test)]
mod normalization_tests {
    use super::*;

    #[test]
    fn d3_allows_only_classes_of_effectively_repaired_fields() {
        let mut d = Diagnostics::default();
        let mask = d.compare(
            Classes::MAPPING | Classes::VIEW,
            Classes::NONE,
            1 << Row::Normalize as usize,
            &[Classes::NONE; oracle::ROW_COUNT],
            false,
            Classes::MAPPING,
        );
        assert_eq!(mask, oracle::Difference::D3.mask() | (1 << 15));
        assert_eq!(d.unreviewed, 1, "unrelated VIEW must not receive a D3 allowance");
        assert_eq!(d.raw, [0, 1, 0, 0, 0, 0, 0, 0, 0, 1]);
    }

    #[test]
    fn d9_requires_a_mask_source_change_and_only_allows_new_acq_and_mask() {
        for row in [Row::MaskSource, Row::Mask, Row::Reset] {
            let mut d = Diagnostics::default();
            let mask = d.compare(
                Classes::ACQ | Classes::MASK | Classes::VIEW,
                Classes::NONE,
                1 << row as usize,
                &[Classes::NONE; oracle::ROW_COUNT],
                false,
                Classes::NONE,
            );
            let source = row == Row::MaskSource;
            assert_eq!(mask, (1 << 15) | if source { oracle::Difference::D9.mask() } else { 0 });
            assert_eq!(d.unreviewed, if source { 1 } else { 3 });
            assert_eq!(d.raw, [1, 0, 0, 0, 1, 0, 0, 0, 0, 1]);
        }
        let mut d = Diagnostics::default();
        let mut old = [Classes::NONE; oracle::ROW_COUNT];
        old[Row::MaskSource as usize] = Classes::ACQ | Classes::MASK;
        assert_eq!(
            d.compare(
                Classes::NONE,
                old[Row::MaskSource as usize],
                1 << Row::MaskSource as usize,
                &old,
                false,
                Classes::NONE
            ),
            1 << 15,
            "D9 cannot excuse missing new acquisition or mask resets"
        );
        assert_eq!(d.unreviewed, 2);
    }
}
