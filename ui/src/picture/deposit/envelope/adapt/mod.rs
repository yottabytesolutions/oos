// SPDX-License-Identifier: GPL-2.0-or-later
//! Sample adapters for the standalone envelope kernel.

#[cfg(test)]
use super::{envelope, ColumnEdges, Correction, Envelopes, LaneMap};

pub(crate) mod boxcar;
pub(crate) mod pairs;

#[cfg(test)]
mod tests;

/// The shared envelope for a contiguous lane whose codes have already been corrected.
#[cfg(test)]
fn reduce_lane(words: &[u16], edges: &ColumnEdges, out: &mut Envelopes<1>) {
    let lanes = LaneMap::new([Some(0), None, None, None], 1).expect("lane 0 maps channel 1");
    envelope(words.as_chunks::<1>().0, &Correction::new([[0]; 8]), edges, &lanes, out);
}
