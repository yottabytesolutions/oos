// SPDX-License-Identifier: GPL-2.0-or-later

#[cfg(any(target_os = "linux", test))]
use super::route;
use super::{clock, decoder, feed::Header, Counters, Lost, SourceWake};
use crate::{board::Frame, model::NCH};
use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

/// Where a frame's read-out holds each channel (`fastdraw::Codes`), as the board gives it (`CodesInto`).
#[derive(Clone, Copy)]
pub struct CodeLayout {
    pub stride: usize,
    pub word: [Option<usize>; NCH],
    pub affine: [(f32, f32); NCH],
    pub(crate) corrected: Option<[crate::fastdraw::CorrectedCodes; NCH]>,
    pub volt: fn(u16, (f32, f32)) -> f32,
    payload: Option<(usize, usize)>,
    reduced_capture: bool,
}

impl CodeLayout {
    pub(crate) fn uncorrected(
        stride: usize,
        word: [Option<usize>; NCH],
        affine: [(f32, f32); NCH],
        volt: fn(u16, (f32, f32)) -> f32,
    ) -> Self {
        Self { stride, word, affine, volt, corrected: None, payload: None, reduced_capture: false }
    }

    pub(crate) fn with_reduced_capture(mut self, reduced: bool) -> Self {
        self.reduced_capture = reduced;
        self
    }

    pub(in crate::drawpipe) fn reduced_capture(&self) -> bool {
        self.reduced_capture
    }

    pub(crate) fn with_payload(mut self, start: usize, end: usize) -> Self {
        self.payload = Some((start, end));
        self
    }

    pub(in crate::drawpipe) fn has_header(&self) -> bool {
        self.payload.is_some_and(|(start, _)| start != 0)
    }

    fn payload<'a>(&self, bytes: &'a [u8]) -> &'a [u8] {
        self.payload.map_or(bytes, |(start, end)| bytes.get(start..end.min(bytes.len())).unwrap_or(&[]))
    }

    pub(in crate::drawpipe) fn points(&self, bytes: &[u8]) -> usize {
        self.payload(bytes).len() / size_of::<u16>() / self.stride
    }

    /// Channel `c` of the read-out `bytes` (empty when not in it).
    pub fn channel<'a>(&self, bytes: &'a [u8], c: usize) -> crate::fastdraw::Codes<'a> {
        let bytes = self.payload(bytes);
        let words = bytes.len() / 2;
        let (word, n) = match self.word[c] {
            Some(k) if k < words => (k, (words - k).div_ceil(self.stride)),
            _ => (0, 0),
        };
        // SAFETY: u16 has no invalid bit patterns; align_to only hands out the aligned middle part
        let (pre, words, _) = unsafe { bytes.align_to::<u16>() };
        let words = (pre.is_empty() && cfg!(target_endian = "little")).then_some(words);
        crate::fastdraw::Codes {
            bytes,
            words,
            stride: self.stride,
            word,
            n,
            affine: self.affine[c],
            corrected: self.corrected.map(|x| x[c]),
            volt: self.volt,
        }
    }
}

/// The board's view of a frame without decoding it: sets every field of the frame but the channel vectors (left
/// empty) as the decode would, and says where the channel words are within its DMA frame.
type ReadCodes = dyn Fn(usize, &[u8], &mut Frame) -> Option<CodeLayout> + Send + Sync;
pub type CodesInto = Arc<ReadCodes>;

/// A read-out buffer of the board (the acquisition thread's DMA target), lent to the workers.
pub trait RawBatch: Send + Sync {
    fn bytes(&self) -> &[u8];
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send>;
    #[cfg(any(target_os = "linux", test))]
    #[doc(hidden)]
    fn take_route_trace(&mut self) -> Option<Box<route::RouteTrace>> {
        None
    }
}

/// Decodes the frame bytes into a kept frame (the board's decoder with the batch's settings).
pub type DecodeInto = Arc<dyn Fn(&[u8], &mut Frame) + Send + Sync>;

/// The buffer while the workers have it: back to the acquisition thread's pool when the last share is done.
pub(super) struct Lease {
    pub(super) raw: Option<Box<dyn RawBatch>>,
    pub(super) home: Sender<Box<dyn RawBatch>>,
    pub(super) source: Arc<SourceWake>,
}

impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(r) = self.raw.take() {
            // the pool is gone only when the pipe is: nothing to return to
            let _ = self.home.send(r);
            self.source.notify();
        }
    }
}

pub(super) enum Message {
    Batch(Arc<Batch>),
    Wake,
    #[cfg(test)]
    Barrier(Sender<()>),
}

pub(super) struct Batch {
    pub(super) fb: usize,
    pub(super) frames: usize,
    pub(super) decode: decoder::Decoder,
    pub(super) codes: Option<CodesInto>,
    pub(super) header: Header,
    pub(super) view_gen_at_submit: u64,
    pub(super) next: AtomicUsize,
    pub(super) counters: &'static Counters,
    pub(super) live: Arc<AtomicUsize>,
    pub(super) newest: Option<Arc<Frame>>,
    pub(super) clock: clock::StageClock,
    pub(super) lease: Lease,
}

impl Drop for Batch {
    fn drop(&mut self) {
        let left = self.frames.saturating_sub(self.next.load(Ordering::Relaxed));
        self.counters.discard(Lost::Unclaimed, left as u64);
        if left > 0 {
            self.counters.metrics.discarded(self.header.identity(), self.clock);
        }
        self.live.fetch_sub(1, Ordering::Release);
    }
}
