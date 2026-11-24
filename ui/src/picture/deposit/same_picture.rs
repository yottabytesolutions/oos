// SPDX-License-Identifier: GPL-2.0-or-later
//! Plane-only fold for a caller-adjudicated same-picture subset. No first-use,
//! retention, ownership transfer or metadata mutation occurs here.

use super::Deposit;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PictureTotals {
    pub(crate) records: u64,
    pub(crate) triggered: u64,
    pub(crate) hits: u64,
    pub(crate) edges: [u64; 4],
}

impl Deposit {
    /// Preflight the entire selected subset before touching any plane. The five
    /// existing physical sets bound inputs; this routine never obtains a set.
    /// Caller must separately adjudicate each positive input at the actual
    /// composition clock, then account its folds, taps and first use exactly once.
    pub(crate) fn merge_picture_planes(&mut self, inputs: &[&Self]) -> Option<PictureTotals> {
        let totals = self.picture_totals(inputs)?;
        for input in inputs {
            let source = input.planes.iter().chain(input.zoom_planes().into_iter().flatten());
            for (out, source) in self.all_bands_mut().zip(source) {
                // Every band was checked before the first mutation.
                assert!(out.merge_picture(source));
            }
            self.add_weights(input.weights());
        }
        Some(totals)
    }

    pub(crate) fn picture_totals(&self, inputs: &[&Self]) -> Option<PictureTotals> {
        if inputs.len() >= 5 {
            return None;
        }
        let mut sum = PictureTotals {
            records: self.records,
            triggered: self.triggered,
            hits: self.extent.hits(),
            edges: self.edges,
        };
        let format = self.format();
        if !self.planes.iter().chain(self.zoom_planes().into_iter().flatten()).all(|band| band.format() == format) {
            return None;
        }
        for input in inputs {
            if self.maps.gen != input.maps.gen
                || self.sizes() != input.sizes()
                || self.identity != input.identity
                || !compatible_bands(&self.planes, &input.planes)
                || !compatible_bands(self.zoom_planes().unwrap_or_default(), input.zoom_planes().unwrap_or_default())
            {
                return None;
            }
            sum.records = sum.records.checked_add(input.records)?;
            sum.triggered = sum.triggered.checked_add(input.triggered)?;
            sum.hits = sum.hits.checked_add(input.extent.hits())?;
            for (total, value) in sum.edges.iter_mut().zip(input.edges) {
                *total = total.checked_add(value)?;
            }
        }
        (sum.hits <= format.hit_limit()).then_some(sum)
    }
}

fn compatible_bands(a: &[super::DiffBand], b: &[super::DiffBand]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.can_merge_picture(b))
}

#[cfg(test)]
mod tests;
