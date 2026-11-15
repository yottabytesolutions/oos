// SPDX-License-Identifier: GPL-2.0-or-later

use crate::math::{MathOp, Window};
use crate::model::Model;

#[derive(Clone, Debug, PartialEq)]
pub enum Analysis0 {
    Math(MathOp),
    MathA(usize),
    MathB(usize),
    MathScale(usize),
    MathFc(f32),
    MathLin((f32, f32)),
    MathExpr([String; crate::math::NMATH]),
    SpecMargin(f32),
    MathOffset(f32),
    FftWin(Window),
    MathBw(f32),
    MathSel(usize),
}

impl Analysis0 {
    pub(super) fn capture(index: usize, m: &Model) -> Option<Self> {
        Some(match index {
            72 => Self::Math(m.math),
            73 => Self::MathA(m.math_a),
            74 => Self::MathB(m.math_b),
            75 => Self::MathScale(m.math_scale),
            76 => Self::MathFc(m.math_fc),
            77 => Self::MathLin(m.math_lin),
            78 => Self::MathExpr(m.math_expr.clone()),
            80 => Self::SpecMargin(m.spec_margin),
            81 => Self::MathOffset(m.math_offset),
            82 => Self::FftWin(m.fft_win),
            83 => Self::MathBw(m.math_bw),
            84 => Self::MathSel(m.math_sel),
            _ => return None,
        })
    }
    pub(super) fn index(&self) -> usize {
        match self {
            Self::Math(_) => 72,
            Self::MathA(_) => 73,
            Self::MathB(_) => 74,
            Self::MathScale(_) => 75,
            Self::MathFc(_) => 76,
            Self::MathLin(_) => 77,
            Self::MathExpr(_) => 78,
            Self::SpecMargin(_) => 80,
            Self::MathOffset(_) => 81,
            Self::FftWin(_) => 82,
            Self::MathBw(_) => 83,
            Self::MathSel(_) => 84,
        }
    }
    pub(super) fn apply(&self, m: &mut Model) {
        match self {
            Self::Math(value) => m.math.clone_from(value),
            Self::MathA(value) => m.math_a.clone_from(value),
            Self::MathB(value) => m.math_b.clone_from(value),
            Self::MathScale(value) => m.math_scale.clone_from(value),
            Self::MathFc(value) => m.math_fc.clone_from(value),
            Self::MathLin(value) => m.math_lin.clone_from(value),
            Self::MathExpr(value) => {
                for (a, b) in m.math_expr.iter_mut().zip(value) {
                    a.clone_from(b);
                }
            }
            Self::SpecMargin(value) => m.spec_margin.clone_from(value),
            Self::MathOffset(value) => m.math_offset.clone_from(value),
            Self::FftWin(value) => m.fft_win.clone_from(value),
            Self::MathBw(value) => m.math_bw.clone_from(value),
            Self::MathSel(value) => m.math_sel.clone_from(value),
        }
    }
    pub(super) fn reuse(&mut self, m: &Model) {
        match self {
            Self::Math(value) => value.clone_from(&m.math),
            Self::MathA(value) => value.clone_from(&m.math_a),
            Self::MathB(value) => value.clone_from(&m.math_b),
            Self::MathScale(value) => value.clone_from(&m.math_scale),
            Self::MathFc(value) => value.clone_from(&m.math_fc),
            Self::MathLin(value) => value.clone_from(&m.math_lin),
            Self::MathExpr(value) => {
                for (a, b) in value.iter_mut().zip(&m.math_expr) {
                    a.clone_from(b);
                }
            }
            Self::SpecMargin(value) => value.clone_from(&m.spec_margin),
            Self::MathOffset(value) => value.clone_from(&m.math_offset),
            Self::FftWin(value) => value.clone_from(&m.fft_win),
            Self::MathBw(value) => value.clone_from(&m.math_bw),
            Self::MathSel(value) => value.clone_from(&m.math_sel),
        }
    }
    pub(super) fn bytes(&self) -> usize {
        match self {
            Self::MathExpr(value) => value.iter().map(String::capacity).sum(),
            _ => 0,
        }
    }
}
