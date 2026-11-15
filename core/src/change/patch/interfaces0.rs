// SPDX-License-Identifier: GPL-2.0-or-later

use crate::model::{AuxOut, GenCh, Model, NGEN};

#[derive(Clone, Debug, PartialEq)]
pub enum Interfaces0 {
    AuxOut(AuxOut),
    GenSel(usize),
    GenBank([GenCh; NGEN]),
    AfgOn(bool),
    AfgArb(u32),
    AfgArbSrc(usize),
    AfgFreq(f64),
    AfgAmpl(f64),
    AfgOffset(f64),
    AfgWave(usize),
    AfgDuty(f64),
    AfgSymmetry(f64),
}

impl Interfaces0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            66 => Self::AuxOut(m.aux_out),
            168 => Self::GenSel(m.gen_sel),
            169 => Self::GenBank(m.gen_bank),
            170 => Self::AfgOn(m.afg_on),
            171 => Self::AfgArb(m.afg_arb),
            172 => Self::AfgArbSrc(m.afg_arb_src),
            173 => Self::AfgFreq(m.afg_freq),
            174 => Self::AfgAmpl(m.afg_ampl),
            175 => Self::AfgOffset(m.afg_offset),
            176 => Self::AfgWave(m.afg_wave),
            177 => Self::AfgDuty(m.afg_duty),
            178 => Self::AfgSymmetry(m.afg_symmetry),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::AuxOut(_) => 66,
            Self::GenSel(_) => 168,
            Self::GenBank(_) => 169,
            Self::AfgOn(_) => 170,
            Self::AfgArb(_) => 171,
            Self::AfgArbSrc(_) => 172,
            Self::AfgFreq(_) => 173,
            Self::AfgAmpl(_) => 174,
            Self::AfgOffset(_) => 175,
            Self::AfgWave(_) => 176,
            Self::AfgDuty(_) => 177,
            Self::AfgSymmetry(_) => 178,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::AuxOut(value) => m.aux_out.clone_from(value),
            Self::GenSel(value) => m.gen_sel.clone_from(value),
            Self::GenBank(value) => m.gen_bank.clone_from(value),
            Self::AfgOn(value) => m.afg_on.clone_from(value),
            Self::AfgArb(value) => m.afg_arb.clone_from(value),
            Self::AfgArbSrc(value) => m.afg_arb_src.clone_from(value),
            Self::AfgFreq(value) => m.afg_freq.clone_from(value),
            Self::AfgAmpl(value) => m.afg_ampl.clone_from(value),
            Self::AfgOffset(value) => m.afg_offset.clone_from(value),
            Self::AfgWave(value) => m.afg_wave.clone_from(value),
            Self::AfgDuty(value) => m.afg_duty.clone_from(value),
            Self::AfgSymmetry(value) => m.afg_symmetry.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::AuxOut(value) => value.clone_from(&m.aux_out),
            Self::GenSel(value) => value.clone_from(&m.gen_sel),
            Self::GenBank(value) => value.clone_from(&m.gen_bank),
            Self::AfgOn(value) => value.clone_from(&m.afg_on),
            Self::AfgArb(value) => value.clone_from(&m.afg_arb),
            Self::AfgArbSrc(value) => value.clone_from(&m.afg_arb_src),
            Self::AfgFreq(value) => value.clone_from(&m.afg_freq),
            Self::AfgAmpl(value) => value.clone_from(&m.afg_ampl),
            Self::AfgOffset(value) => value.clone_from(&m.afg_offset),
            Self::AfgWave(value) => value.clone_from(&m.afg_wave),
            Self::AfgDuty(value) => value.clone_from(&m.afg_duty),
            Self::AfgSymmetry(value) => value.clone_from(&m.afg_symmetry),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        0
    }
}
