// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::Model;

#[derive(Clone, Debug, PartialEq)]
pub enum ReferencesP1 {
    MaskInv(bool),
}

impl ReferencesP1 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            256 => Self::MaskInv(m.mask_inv),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::MaskInv(_) => 256,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::MaskInv(value) => m.mask_inv.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::MaskInv(value) => value.clone_from(&m.mask_inv),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
