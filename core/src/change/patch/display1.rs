// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{AView, DotsMode, HdmiMode, Hist, Model, TextSize, VIEWS_MAX};

#[derive(Clone, Debug, PartialEq)]
pub enum Display1 {
    Dots(DotsMode),
    Hdmi(HdmiMode),
    Views(usize),
    ViewCur(usize),
    ViewSlots([String; VIEWS_MAX]),
    ZSrc(Option<usize>),
    Hist(Hist),
    View(AView),
    TextSize(TextSize),
}

impl Display1 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            122 => Self::Dots(m.dots),
            123 => Self::Hdmi(m.hdmi),
            124 => Self::Views(m.views),
            125 => Self::ViewCur(m.view_cur),
            126 => Self::ViewSlots(m.view_slots.clone()),
            127 => Self::ZSrc(m.z_src),
            128 => Self::Hist(m.hist),
            129 => Self::View(m.view),
            192 => Self::TextSize(m.text_size),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::Dots(_) => 122,
            Self::Hdmi(_) => 123,
            Self::Views(_) => 124,
            Self::ViewCur(_) => 125,
            Self::ViewSlots(_) => 126,
            Self::ZSrc(_) => 127,
            Self::Hist(_) => 128,
            Self::View(_) => 129,
            Self::TextSize(_) => 192,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::Dots(value) => m.dots.clone_from(value),
            Self::Hdmi(value) => m.hdmi.clone_from(value),
            Self::Views(value) => m.views.clone_from(value),
            Self::ViewCur(value) => m.view_cur.clone_from(value),
            Self::ViewSlots(value) => {
                for (a, b) in m.view_slots.iter_mut().zip(value) {
                    a.clone_from(b);
                }
            }
            Self::ZSrc(value) => m.z_src.clone_from(value),
            Self::Hist(value) => m.hist.clone_from(value),
            Self::View(value) => m.view.clone_from(value),
            Self::TextSize(value) => m.text_size.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::Dots(value) => value.clone_from(&m.dots),
            Self::Hdmi(value) => value.clone_from(&m.hdmi),
            Self::Views(value) => value.clone_from(&m.views),
            Self::ViewCur(value) => value.clone_from(&m.view_cur),
            Self::ViewSlots(value) => {
                for (a, b) in value.iter_mut().zip(&m.view_slots) {
                    a.clone_from(b);
                }
            }
            Self::ZSrc(value) => value.clone_from(&m.z_src),
            Self::Hist(value) => value.clone_from(&m.hist),
            Self::View(value) => value.clone_from(&m.view),
            Self::TextSize(value) => value.clone_from(&m.text_size),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::ViewSlots(value) => value.iter().map(String::capacity).sum(),
            _ => 0,
        }
    }
}
