// SPDX-License-Identifier: GPL-2.0-or-later

use crate::board::Frame;
use std::sync::Arc;

/// A frame decoder with immutable preparation indexed by record order in its batch.
pub type IndexedDecodeInto = Arc<dyn Fn(usize, &[u8], &mut Frame) + Send + Sync>;

pub(super) enum Decoder {
    Plain(super::DecodeInto),
    #[cfg(any(target_os = "linux", test))]
    Prepared(IndexedDecodeInto),
}

impl Decoder {
    #[cfg_attr(not(any(target_os = "linux", test)), allow(unused_variables))] // only prepared decoders use the index
    pub(super) fn decode(&self, index: usize, bytes: &[u8], frame: &mut Frame) {
        match self {
            Self::Plain(decode) => decode(bytes, frame),
            #[cfg(any(target_os = "linux", test))]
            Self::Prepared(decode) => decode(index, bytes, frame),
        }
    }
}

impl From<super::DecodeInto> for Decoder {
    fn from(decode: super::DecodeInto) -> Self {
        Self::Plain(decode)
    }
}
