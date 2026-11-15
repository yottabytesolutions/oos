// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{HExpand, Model};

#[derive(Clone, Debug, PartialEq)]
pub enum Timebase0 {
    Tscale(usize),
    Delay(f32),
    FineT(bool),
    Tfine(f32),
    HExpand(HExpand),
    Zoom(bool),
    Zscale(usize),
    Zdelay(f32),
    AutoRoll(bool),
}

impl Timebase0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            2 => Self::Tscale(m.tscale),
            3 => Self::Delay(m.delay),
            68 => Self::FineT(m.fine_t),
            69 => Self::Tfine(m.tfine),
            71 => Self::HExpand(m.h_expand),
            117 => Self::Zoom(m.zoom),
            119 => Self::Zscale(m.zscale),
            120 => Self::Zdelay(m.zdelay),
            118 => Self::AutoRoll(m.auto_roll),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::Tscale(_) => 2,
            Self::Delay(_) => 3,
            Self::FineT(_) => 68,
            Self::Tfine(_) => 69,
            Self::HExpand(_) => 71,
            Self::Zoom(_) => 117,
            Self::AutoRoll(_) => 118,
            Self::Zscale(_) => 119,
            Self::Zdelay(_) => 120,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::Tscale(value) => m.tscale.clone_from(value),
            Self::Delay(value) => m.delay.clone_from(value),
            Self::FineT(value) => m.fine_t.clone_from(value),
            Self::Tfine(value) => m.tfine.clone_from(value),
            Self::HExpand(value) => m.h_expand.clone_from(value),
            Self::Zoom(value) => m.zoom.clone_from(value),
            Self::Zscale(value) => m.zscale.clone_from(value),
            Self::Zdelay(value) => m.zdelay.clone_from(value),
            Self::AutoRoll(value) => m.auto_roll.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::Tscale(value) => value.clone_from(&m.tscale),
            Self::Delay(value) => value.clone_from(&m.delay),
            Self::FineT(value) => value.clone_from(&m.fine_t),
            Self::Tfine(value) => value.clone_from(&m.tfine),
            Self::HExpand(value) => value.clone_from(&m.h_expand),
            Self::Zoom(value) => value.clone_from(&m.zoom),
            Self::Zscale(value) => value.clone_from(&m.zscale),
            Self::Zdelay(value) => value.clone_from(&m.zdelay),
            Self::AutoRoll(value) => value.clone_from(&m.auto_roll),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
