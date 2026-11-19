// SPDX-License-Identifier: GPL-2.0-or-later

use crate::wave::Phosphor;
use std::ops::Range;

pub(super) enum UploadData<'a> {
    Pixels { bytes: &'a [u8], row_bytes: usize },
    Channel { phosphor: &'a Phosphor, channel: usize },
}

impl<'a> UploadData<'a> {
    pub(super) fn counts(phosphor: &'a Phosphor, channel: usize, encoded: &'a [u16]) -> Self {
        if phosphor.is_wide() {
            Self::Pixels { bytes: super::bytes(encoded), row_bytes: phosphor.w * 2 }
        } else {
            Self::Channel { phosphor, channel }
        }
    }

    pub(super) fn bytes(&self) -> &[u8] {
        match self {
            Self::Pixels { bytes, .. } => bytes,
            Self::Channel { phosphor, channel } => super::bytes(phosphor.maps()[*channel].as_slice()),
        }
    }

    pub(super) fn row_bytes(&self) -> usize {
        match self {
            Self::Pixels { row_bytes, .. } => *row_bytes,
            Self::Channel { phosphor, .. } => phosphor.w * 2,
        }
    }
}

pub(super) struct Block {
    offset: usize,
    len: usize,
    stamp: Option<crate::wave::dirty::Stamp>,
}

impl Block {
    pub(super) fn matches(&self, offset: usize, len: usize) -> bool {
        (self.offset, self.len) == (offset, len)
    }

    pub(super) fn publish(
        offset: usize,
        dst: &mut [u8],
        source: UploadData<'_>,
        previous: Option<&Self>,
    ) -> (Self, Range<usize>) {
        let src = source.bytes();
        let row = source.row_bytes();
        if src.is_empty() {
            return (Self { offset, len: dst.len(), stamp: None }, copy_rows(dst, src, row, None));
        }
        debug_assert!(row > 0);
        let (stamp, dirty) = match &source {
            UploadData::Channel { phosphor, channel } => {
                let (stamp, band) = phosphor.upload_rows(*channel, previous.and_then(|block| block.stamp.as_ref()));
                (Some(stamp), Some(band))
            }
            UploadData::Pixels { .. } => (None, previous.is_none().then_some(0..src.len() / row)),
        };
        let band = copy_rows(dst, src, row, dirty);
        (Self { offset, len: dst.len(), stamp }, band)
    }
}

/// Known dirty bands are copied once; legacy pixel layers still discover their band by comparison.
pub(super) fn copy_rows(dst: &mut [u8], src: &[u8], row: usize, dirty: Option<Range<usize>>) -> Range<usize> {
    assert_eq!(dst.len(), src.len());
    if src.is_empty() {
        return 0..0;
    }
    debug_assert!(row > 0);
    if let Some(band) = dirty {
        let bytes = band.start * row..band.end * row;
        dst[bytes.clone()].copy_from_slice(&src[bytes]);
        return band;
    }
    let mut band = 0..0;
    for (y, (dst, src)) in dst.chunks_mut(row).zip(src.chunks(row)).enumerate() {
        if super::differ(dst, src) {
            if band.is_empty() {
                band.start = y;
            }
            band.end = y + 1;
            dst.copy_from_slice(src);
        }
    }
    band
}

#[cfg(test)]
mod tests;
