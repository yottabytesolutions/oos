// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{Channel, Model, NCH};

#[derive(Clone, Debug, PartialEq)]
pub enum Channels0 {
    Ch([Channel; NCH]),
    Active(usize),
    ChLabel([String; NCH]),
    FineV(bool),
    VExpandGnd(bool),
}

impl Channels0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            0 => Self::Ch(m.ch.clone()),
            1 => Self::Active(m.active),
            59 => Self::ChLabel(m.ch_label.clone()),
            67 => Self::FineV(m.fine_v),
            70 => Self::VExpandGnd(m.v_expand_gnd),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::Ch(_) => 0,
            Self::Active(_) => 1,
            Self::ChLabel(_) => 59,
            Self::FineV(_) => 67,
            Self::VExpandGnd(_) => 70,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::Ch(value) => m.ch.clone_from(value),
            Self::Active(value) => m.active.clone_from(value),
            Self::ChLabel(value) => {
                for (a, b) in m.ch_label.iter_mut().zip(value) {
                    a.clone_from(b);
                }
            }
            Self::FineV(value) => m.fine_v.clone_from(value),
            Self::VExpandGnd(value) => m.v_expand_gnd.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::Ch(value) => value.clone_from(&m.ch),
            Self::Active(value) => value.clone_from(&m.active),
            Self::ChLabel(value) => {
                for (a, b) in value.iter_mut().zip(&m.ch_label) {
                    a.clone_from(b);
                }
            }
            Self::FineV(value) => value.clone_from(&m.fine_v),
            Self::VExpandGnd(value) => value.clone_from(&m.v_expand_gnd),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::ChLabel(value) => value.iter().map(String::capacity).sum(),
            _ => 0,
        }
    }
}
