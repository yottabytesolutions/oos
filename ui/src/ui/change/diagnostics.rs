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
    event_first: [u16; 16],
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
            event_first: [0; 16],
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
        by_row: &[Classes; 16],
        stopped: bool,
    ) -> u8 {
        let mut differences = 0;
        for (i, c) in Classes::ALL.into_iter().enumerate() {
            if new.contains(c) == old.contains(c) {
                continue;
            }
            self.raw[i] += 1;
            let found = Row::ALL
                .into_iter()
                .filter(|r| rows & (1 << *r as usize) != 0 && (!old.contains(c) || by_row[*r as usize].contains(c)))
                .find_map(|r| oracle::difference(r, c, old.contains(c), stopped).map(|d| (r, d)));
            let (row, difference) = found.unwrap_or((Row::Drawing, "unreviewed"));
            if found.is_none() {
                self.unreviewed += 1;
                differences |= 128;
            } else {
                differences |= match difference {
                    "D1" => 1,
                    "D2" => 2,
                    "D3" => 4,
                    "D4" => 8,
                    "D5" => 16,
                    _ => 32,
                };
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
    pub(super) fn field(&mut self, f: FieldDiff, row: Row, differences: u8) {
        if f.changed() && !self.first[f.index()] {
            self.first[f.index()] = true;
            // The fixed mask is D1..D6 in bits 0..5; bit 7 is unreviewed. Class logs spell out each id.
            eprintln!(
                "oos-ui shadow: first field={} row={} classes={:?} difference_ids=0x{:02x}",
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
