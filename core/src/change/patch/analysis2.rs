// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{Model, Proto};

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis2 {
    DecUspeed(usize),
    DecSrc4(Option<usize>),
    DecLaIn(bool),
    DecLa([Option<u8>; 4]),
    DecDiff(Option<usize>),
    DecThr(f32),
    DecAscii(bool),
    DecProto(Proto),
    DecSrc2(usize),
    DecCs(Option<usize>),
    DecMode(u8),
    DecBits(u32),
}

impl Analysis2 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            100 => Self::DecUspeed(m.dec_uspeed),
            101 => Self::DecSrc4(m.dec_src4),
            102 => Self::DecLaIn(m.dec_la_in),
            103 => Self::DecLa(m.dec_la),
            104 => Self::DecDiff(m.dec_diff),
            108 => Self::DecThr(m.dec_thr),
            109 => Self::DecAscii(m.dec_ascii),
            110 => Self::DecProto(m.dec_proto),
            111 => Self::DecSrc2(m.dec_src2),
            112 => Self::DecCs(m.dec_cs),
            113 => Self::DecMode(m.dec_mode),
            114 => Self::DecBits(m.dec_bits),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::DecUspeed(_) => 100,
            Self::DecSrc4(_) => 101,
            Self::DecLaIn(_) => 102,
            Self::DecLa(_) => 103,
            Self::DecDiff(_) => 104,
            Self::DecThr(_) => 108,
            Self::DecAscii(_) => 109,
            Self::DecProto(_) => 110,
            Self::DecSrc2(_) => 111,
            Self::DecCs(_) => 112,
            Self::DecMode(_) => 113,
            Self::DecBits(_) => 114,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::DecUspeed(value) => m.dec_uspeed.clone_from(value),
            Self::DecSrc4(value) => m.dec_src4.clone_from(value),
            Self::DecLaIn(value) => m.dec_la_in.clone_from(value),
            Self::DecLa(value) => m.dec_la.clone_from(value),
            Self::DecDiff(value) => m.dec_diff.clone_from(value),
            Self::DecThr(value) => m.dec_thr.clone_from(value),
            Self::DecAscii(value) => m.dec_ascii.clone_from(value),
            Self::DecProto(value) => m.dec_proto.clone_from(value),
            Self::DecSrc2(value) => m.dec_src2.clone_from(value),
            Self::DecCs(value) => m.dec_cs.clone_from(value),
            Self::DecMode(value) => m.dec_mode.clone_from(value),
            Self::DecBits(value) => m.dec_bits.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::DecUspeed(value) => value.clone_from(&m.dec_uspeed),
            Self::DecSrc4(value) => value.clone_from(&m.dec_src4),
            Self::DecLaIn(value) => value.clone_from(&m.dec_la_in),
            Self::DecLa(value) => value.clone_from(&m.dec_la),
            Self::DecDiff(value) => value.clone_from(&m.dec_diff),
            Self::DecThr(value) => value.clone_from(&m.dec_thr),
            Self::DecAscii(value) => value.clone_from(&m.dec_ascii),
            Self::DecProto(value) => value.clone_from(&m.dec_proto),
            Self::DecSrc2(value) => value.clone_from(&m.dec_src2),
            Self::DecCs(value) => value.clone_from(&m.dec_cs),
            Self::DecMode(value) => value.clone_from(&m.dec_mode),
            Self::DecBits(value) => value.clone_from(&m.dec_bits),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
