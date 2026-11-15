// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{Model, Slope};

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis5 {
    BodeGen(usize),
    BodeAddr(String),
    BodeUsb(bool),
    BodeSwTime(f64),
    BodeSwLog(bool),
    BodeSync(usize),
    BodeSyncSlope(Slope),
    BodeSyncLevel(f32),
    BodeSwIn(usize),
    BodeRef(f64),
    BwSrc(usize),
    BwHint(f64),
}

impl Analysis5 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            152 => Self::BodeGen(m.bode_gen),
            153 => Self::BodeAddr(m.bode_addr.clone()),
            154 => Self::BodeUsb(m.bode_usb),
            155 => Self::BodeSwTime(m.bode_sw_time),
            156 => Self::BodeSwLog(m.bode_sw_log),
            157 => Self::BodeSync(m.bode_sync),
            158 => Self::BodeSyncSlope(m.bode_sync_slope),
            159 => Self::BodeSyncLevel(m.bode_sync_level),
            160 => Self::BodeSwIn(m.bode_sw_in),
            161 => Self::BodeRef(m.bode_ref),
            165 => Self::BwSrc(m.bw_src),
            166 => Self::BwHint(m.bw_hint),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::BodeGen(_) => 152,
            Self::BodeAddr(_) => 153,
            Self::BodeUsb(_) => 154,
            Self::BodeSwTime(_) => 155,
            Self::BodeSwLog(_) => 156,
            Self::BodeSync(_) => 157,
            Self::BodeSyncSlope(_) => 158,
            Self::BodeSyncLevel(_) => 159,
            Self::BodeSwIn(_) => 160,
            Self::BodeRef(_) => 161,
            Self::BwSrc(_) => 165,
            Self::BwHint(_) => 166,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::BodeGen(value) => m.bode_gen.clone_from(value),
            Self::BodeAddr(value) => m.bode_addr.clone_from(value),
            Self::BodeUsb(value) => m.bode_usb.clone_from(value),
            Self::BodeSwTime(value) => m.bode_sw_time.clone_from(value),
            Self::BodeSwLog(value) => m.bode_sw_log.clone_from(value),
            Self::BodeSync(value) => m.bode_sync.clone_from(value),
            Self::BodeSyncSlope(value) => m.bode_sync_slope.clone_from(value),
            Self::BodeSyncLevel(value) => m.bode_sync_level.clone_from(value),
            Self::BodeSwIn(value) => m.bode_sw_in.clone_from(value),
            Self::BodeRef(value) => m.bode_ref.clone_from(value),
            Self::BwSrc(value) => m.bw_src.clone_from(value),
            Self::BwHint(value) => m.bw_hint.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::BodeGen(value) => value.clone_from(&m.bode_gen),
            Self::BodeAddr(value) => value.clone_from(&m.bode_addr),
            Self::BodeUsb(value) => value.clone_from(&m.bode_usb),
            Self::BodeSwTime(value) => value.clone_from(&m.bode_sw_time),
            Self::BodeSwLog(value) => value.clone_from(&m.bode_sw_log),
            Self::BodeSync(value) => value.clone_from(&m.bode_sync),
            Self::BodeSyncSlope(value) => value.clone_from(&m.bode_sync_slope),
            Self::BodeSyncLevel(value) => value.clone_from(&m.bode_sync_level),
            Self::BodeSwIn(value) => value.clone_from(&m.bode_sw_in),
            Self::BodeRef(value) => value.clone_from(&m.bode_ref),
            Self::BwSrc(value) => value.clone_from(&m.bw_src),
            Self::BwHint(value) => value.clone_from(&m.bw_hint),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::BodeAddr(value) => value.capacity(),
            _ => 0,
        }
    }
}
