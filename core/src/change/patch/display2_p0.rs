// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::Model;

#[derive(Clone, Debug, PartialEq)]
pub enum Display2P0 {
    DrawBatch(bool),
    BarCompact(bool),
    HdmiView(usize),
}

impl Display2P0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            342 => Self::DrawBatch(m.draw_batch),
            343 => Self::BarCompact(m.bar_compact),
            344 => Self::HdmiView(m.hdmi_view),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::DrawBatch(_) => 342,
            Self::BarCompact(_) => 343,
            Self::HdmiView(_) => 344,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::DrawBatch(value) => m.draw_batch.clone_from(value),
            Self::BarCompact(value) => m.bar_compact.clone_from(value),
            Self::HdmiView(value) => m.hdmi_view.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::DrawBatch(value) => value.clone_from(&m.draw_batch),
            Self::BarCompact(value) => value.clone_from(&m.bar_compact),
            Self::HdmiView(value) => value.clone_from(&m.hdmi_view),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
