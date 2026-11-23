// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) fn newest_at(b: &Batch, index: usize) -> Option<&Frame> {
    (index + 1 == b.frames).then(|| newest(b)).flatten().map(Arc::as_ref)
}

pub(super) fn decoded_newest_at(b: &Batch, index: usize) -> Option<&Frame> {
    newest_at(b, index).filter(|_| b.header.newest_from_dma || b.codes.is_none())
}

pub(super) fn newest(b: &Batch) -> Option<&Arc<Frame>> {
    b.newest.as_ref()
}

pub(super) fn chunk(b: &Batch, i: usize) -> &[u8] {
    let i = b.header.dma_slot(i);
    &b.lease.raw.as_ref().expect("batch owns its buffer until every job returns").bytes()[i * b.fb..(i + 1) * b.fb]
}

pub(super) fn fold_codes(
    scratch: &mut Frame,
    m: &Model,
    (bytes, raw): (&[u8], CodeLayout),
    folds: &mut crate::framestats::Folds,
    decode: impl FnOnce(&mut Frame),
) -> bool {
    if let Some(measured) = {
        #[cfg(test)]
        let measured = fixture_bench::sample_fold(folds, |folds| try_fold_codes(scratch, m, bytes, raw, folds));
        #[cfg(not(test))]
        let measured = try_fold_codes(scratch, m, bytes, raw, folds);
        measured
    } {
        return measured;
    }
    Cause::MeasurementDecode.mark();
    decode(scratch);
    fold_decoded(scratch, m, folds)
}

fn try_fold_codes(
    frame: &Frame,
    m: &Model,
    bytes: &[u8],
    raw: CodeLayout,
    folds: &mut crate::framestats::Folds,
) -> Option<bool> {
    if crate::framestats::requires_frame(m, frame, folds) {
        None
    } else {
        crate::framestats::fold_codes(frame, m, bytes, raw, folds)
    }
}

pub(super) fn fold_decoded(frame: &Frame, m: &Model, folds: &mut crate::framestats::Folds) -> bool {
    #[cfg(test)]
    return fixture_bench::sample_fold(folds, |folds| fold_frame(frame, m, folds));
    #[cfg(not(test))]
    fold_frame(frame, m, folds)
}

fn fold_frame(frame: &Frame, m: &Model, folds: &mut crate::framestats::Folds) -> bool {
    if let Some(ti) = route::hardware_position(m, frame) {
        crate::framestats::fold_frame_with(frame, m, folds, Some(ti))
    } else {
        crate::framestats::fold_frame(frame, m, folds)
    }
}
