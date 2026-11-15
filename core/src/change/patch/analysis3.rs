// SPDX-License-Identifier: GPL-2.0-or-later

use crate::math::Window;
use crate::model::Model;

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis3 {
    DecParity(oos_dsp::decode::Parity),
    DecOpt([u8; crate::protocols::OPTS]),
    SpecSrc(usize),
    SpecCenter(f64),
    SpecSpan(f64),
    SpecRbw(f64),
    SpecWin(Window),
    SpecSort(usize),
    SpecHarm(usize),
    SpecTrace(usize),
    SpecRef(f32),
    SpecWf(bool),
}

impl Analysis3 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            115 => Self::DecParity(m.dec_parity),
            116 => Self::DecOpt(m.dec_opt),
            130 => Self::SpecSrc(m.spec_src),
            131 => Self::SpecCenter(m.spec_center),
            132 => Self::SpecSpan(m.spec_span),
            133 => Self::SpecRbw(m.spec_rbw),
            134 => Self::SpecWin(m.spec_win),
            135 => Self::SpecSort(m.spec_sort),
            136 => Self::SpecHarm(m.spec_harm),
            137 => Self::SpecTrace(m.spec_trace),
            138 => Self::SpecRef(m.spec_ref),
            139 => Self::SpecWf(m.spec_wf),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::DecParity(_) => 115,
            Self::DecOpt(_) => 116,
            Self::SpecSrc(_) => 130,
            Self::SpecCenter(_) => 131,
            Self::SpecSpan(_) => 132,
            Self::SpecRbw(_) => 133,
            Self::SpecWin(_) => 134,
            Self::SpecSort(_) => 135,
            Self::SpecHarm(_) => 136,
            Self::SpecTrace(_) => 137,
            Self::SpecRef(_) => 138,
            Self::SpecWf(_) => 139,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::DecParity(value) => m.dec_parity.clone_from(value),
            Self::DecOpt(value) => m.dec_opt.clone_from(value),
            Self::SpecSrc(value) => m.spec_src.clone_from(value),
            Self::SpecCenter(value) => m.spec_center.clone_from(value),
            Self::SpecSpan(value) => m.spec_span.clone_from(value),
            Self::SpecRbw(value) => m.spec_rbw.clone_from(value),
            Self::SpecWin(value) => m.spec_win.clone_from(value),
            Self::SpecSort(value) => m.spec_sort.clone_from(value),
            Self::SpecHarm(value) => m.spec_harm.clone_from(value),
            Self::SpecTrace(value) => m.spec_trace.clone_from(value),
            Self::SpecRef(value) => m.spec_ref.clone_from(value),
            Self::SpecWf(value) => m.spec_wf.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::DecParity(value) => value.clone_from(&m.dec_parity),
            Self::DecOpt(value) => value.clone_from(&m.dec_opt),
            Self::SpecSrc(value) => value.clone_from(&m.spec_src),
            Self::SpecCenter(value) => value.clone_from(&m.spec_center),
            Self::SpecSpan(value) => value.clone_from(&m.spec_span),
            Self::SpecRbw(value) => value.clone_from(&m.spec_rbw),
            Self::SpecWin(value) => value.clone_from(&m.spec_win),
            Self::SpecSort(value) => value.clone_from(&m.spec_sort),
            Self::SpecHarm(value) => value.clone_from(&m.spec_harm),
            Self::SpecTrace(value) => value.clone_from(&m.spec_trace),
            Self::SpecRef(value) => value.clone_from(&m.spec_ref),
            Self::SpecWf(value) => value.clone_from(&m.spec_wf),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
