// SPDX-License-Identifier: GPL-2.0-or-later

//! Board-owned port for deterministic host read-outs. Layout and decoding stay in the adapter.

use super::{Active, Frame};

#[cfg(test)]
pub(crate) use super::dho::scripted::deep_intake_frame;
use crate::drawpipe::{CodesInto, DecodeInto, RawBatch};
use crate::model::Model;

pub(crate) fn core_matching_probe(sample: impl FnMut(&str, &mut dyn FnMut())) {
    super::dho::scripted::core_matching_probe(sample);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchKind {
    Raw,
    Codes,
    LivePairs,
    DeepPairs,
    Roll,
    Scan,
    HardwareAverage,
}

impl BatchKind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "raw" => Ok(Self::Raw),
            "codes" => Ok(Self::Codes),
            "pairs" => Ok(Self::LivePairs),
            "deep-pairs" => Ok(Self::DeepPairs),
            "roll" => Ok(Self::Roll),
            "scan" => Ok(Self::Scan),
            "hw-average" => Ok(Self::HardwareAverage),
            _ => Err(format!("unknown scripted batch kind {}", value)),
        }
    }
}

/// The active board chooses its host source, just as `Board` chooses its acquisition request types.
pub trait ScriptedBoard {
    type Source: BatchSource;
    fn scripted_source(kind: BatchKind, seed: u64) -> Self::Source;
}

pub type ScriptedBatchSource = <Active as ScriptedBoard>::Source;
pub type ScriptBytes = <ScriptedBatchSource as BatchSource>::Bytes;

pub fn source(kind: BatchKind, seed: u64) -> ScriptedBatchSource {
    Active::scripted_source(kind, seed)
}

pub trait BatchSource {
    type Bytes: RawBatch;
    type Batch: Batch<Bytes = Self::Bytes>;

    /// Produce at most 32 frames and 32 MiB, optionally reusing a returned worker buffer.
    fn next_batch(
        &mut self,
        model: &Model,
        switches: &crate::sysconf::Switches,
        frames: usize,
        reuse: Option<Self::Bytes>,
    ) -> Result<Self::Batch, String>;
}

/// Captured decode metadata and ownership of one complete read-out.
pub trait Batch {
    type Bytes: RawBatch;
    fn frame_bytes(&self) -> usize;
    fn newest(&self, stamp: u64) -> Frame;
    fn frame(&self, index: usize, stamp: u64) -> Frame;
    fn decode(&self) -> DecodeInto;
    fn codes(&self) -> Option<CodesInto>;
    fn into_bytes(self) -> Self::Bytes;
}

#[cfg(test)]
pub(crate) use super::dho::hw::single_fake::{SegmentWorker, SingleWorker};

#[cfg(test)]
mod tests {
    use super::*;

    fn check_contract<S: BatchSource>(mut source: S) {
        let batch = source.next_batch(&Model::default(), &Default::default(), 3, None).unwrap();
        let expected = batch.newest(7);
        let fb = batch.frame_bytes();
        let decode = batch.decode();
        let bytes = batch.into_bytes();
        assert_eq!(bytes.bytes().len(), 3 * fb);
        let mut frame = Frame::default();
        decode(&bytes.bytes()[2 * fb..], &mut frame);
        assert_eq!(frame.ch, expected.ch);
        assert_eq!(frame.acq, expected.acq);
        let next = source.next_batch(&Model::default(), &Default::default(), 3, Some(bytes)).unwrap();
        assert_eq!(next.newest(8).seq, 6);
    }

    #[test]
    fn active_board_implements_the_scripted_source_contract() {
        for kind in [BatchKind::Raw, BatchKind::Codes, BatchKind::LivePairs, BatchKind::DeepPairs] {
            check_contract(source(kind, 123));
        }
        assert!(BatchKind::parse("invalid").is_err());
    }
}
