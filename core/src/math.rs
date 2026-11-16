// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

pub use oos_dsp::math::Window;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MathOp {
    Off,
    Add,
    Sub,
    Mul,
    Div,
    Intg,
    Diff,
    Abs,
    Sqrt,
    Lg,
    Ln,
    Exp,
    /// A x + B
    Lin,
    /// a formula over the channels (`oos_dsp::expr`)
    Expr,
    Lpf,
    Hpf,
    Bpf,
    Bsf,
    Fft,
    Fir,
    And,
    Or,
    Xor,
    Nand,
    Nor,
    Not,
    A2d,
    D2a,
    Conv,
    Unnrzi,
}

/// Math menu categories: Arithmetic, Function, Filter, Logic, Convert.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MathFamily {
    Arith,
    Fn,
    Filter,
    Logic,
    Convert,
}

impl MathFamily {
    pub const ALL: [MathFamily; 5] =
        [MathFamily::Arith, MathFamily::Fn, MathFamily::Filter, MathFamily::Logic, MathFamily::Convert];

    pub fn name(self) -> &'static str {
        match self {
            MathFamily::Arith => "Arithmetic",
            MathFamily::Fn => "Function",
            MathFamily::Filter => "Filter",
            MathFamily::Logic => "Logic",
            MathFamily::Convert => "Convert",
        }
    }

    /// Operations of this category (not Off).
    pub fn ops(self) -> &'static [MathOp] {
        match self {
            MathFamily::Arith => &[MathOp::Add, MathOp::Sub, MathOp::Mul, MathOp::Div, MathOp::Conv],
            MathFamily::Fn => &[
                MathOp::Intg,
                MathOp::Diff,
                MathOp::Abs,
                MathOp::Sqrt,
                MathOp::Lg,
                MathOp::Ln,
                MathOp::Exp,
                MathOp::Lin,
                MathOp::Expr,
            ],
            MathFamily::Filter => &[MathOp::Lpf, MathOp::Hpf, MathOp::Bpf, MathOp::Bsf, MathOp::Fir, MathOp::Fft],
            MathFamily::Logic => &[MathOp::And, MathOp::Or, MathOp::Xor, MathOp::Nand, MathOp::Nor, MathOp::Not],
            MathFamily::Convert => &[MathOp::A2d, MathOp::D2a, MathOp::Unnrzi],
        }
    }
}

pub const MATH_OPS: [MathOp; 30] = [
    MathOp::Off,
    MathOp::Add,
    MathOp::Sub,
    MathOp::Mul,
    MathOp::Div,
    MathOp::Conv,
    MathOp::Intg,
    MathOp::Diff,
    MathOp::Abs,
    MathOp::Sqrt,
    MathOp::Lg,
    MathOp::Ln,
    MathOp::Exp,
    MathOp::Lin,
    MathOp::Expr,
    MathOp::Lpf,
    MathOp::Hpf,
    MathOp::Bpf,
    MathOp::Bsf,
    MathOp::Fir,
    MathOp::Fft,
    MathOp::And,
    MathOp::Or,
    MathOp::Xor,
    MathOp::Nand,
    MathOp::Nor,
    MathOp::Not,
    MathOp::A2d,
    MathOp::D2a,
    MathOp::Unnrzi,
];

/// Math channels (8, math on math).
pub const NMATH: usize = 8;
/// Keep existing CH and Math setup indices stable; references follow M8 (`model::NCH` is `oos_dsp::CHANNELS`).
pub const MATH_REF_BASE: usize = oos_dsp::CHANNELS + NMATH;

/// Name of a math source: 0..3 = CH1..CH4, 4.. = M1..M8, then R1.. (refs).
pub fn src_name(i: usize) -> String {
    if i < 4 {
        format!("CH{}", i + 1)
    } else if i < MATH_REF_BASE {
        format!("M{}", i - 3)
    } else {
        format!("R{}", i - MATH_REF_BASE + 1)
    }
}

pub const WINDOWS: [Window; 5] = [Window::Rect, Window::Hann, Window::Hamming, Window::Blackman, Window::Flattop];

impl MathOp {
    pub fn name(self) -> &'static str {
        match self {
            MathOp::Off => "Off",
            MathOp::Add => "A+B",
            MathOp::Sub => "A-B",
            MathOp::Mul => "AxB",
            MathOp::Div => "A/B",
            MathOp::Intg => "Intg",
            MathOp::Diff => "Diff",
            MathOp::Abs => "Abs",
            MathOp::Sqrt => "Sqrt",
            MathOp::Lg => "Lg",
            MathOp::Ln => "Ln",
            MathOp::Lin => "Ax+B",
            MathOp::Expr => "Formula",
            MathOp::Exp => "Exp",
            MathOp::Lpf => "Low-pass",
            MathOp::Hpf => "High-pass",
            MathOp::Bpf => "Band-pass",
            MathOp::Bsf => "Band-stop",
            MathOp::Fft => "FFT",
            MathOp::Fir => "FIR",
            MathOp::And => "AND",
            MathOp::Or => "OR",
            MathOp::Xor => "XOR",
            MathOp::Nand => "NAND",
            MathOp::Nor => "NOR",
            MathOp::Not => "NOT",
            MathOp::A2d => "A2D",
            MathOp::D2a => "D2A",
            MathOp::Conv => "Conv",
            MathOp::Unnrzi => "NRZI inv",
        }
    }
    pub fn family(self) -> MathFamily {
        match self {
            MathOp::Off | MathOp::Add | MathOp::Sub | MathOp::Mul | MathOp::Div | MathOp::Conv => MathFamily::Arith,
            MathOp::Intg
            | MathOp::Diff
            | MathOp::Abs
            | MathOp::Sqrt
            | MathOp::Lg
            | MathOp::Ln
            | MathOp::Exp
            | MathOp::Lin
            | MathOp::Expr => MathFamily::Fn,
            MathOp::Lpf | MathOp::Hpf | MathOp::Bpf | MathOp::Bsf | MathOp::Fir | MathOp::Fft => MathFamily::Filter,
            MathOp::And | MathOp::Or | MathOp::Xor | MathOp::Nand | MathOp::Nor | MathOp::Not => MathFamily::Logic,
            MathOp::A2d | MathOp::D2a | MathOp::Unnrzi => MathFamily::Convert,
        }
    }
    pub fn two_sources(self) -> bool {
        matches!(
            self,
            MathOp::Add
                | MathOp::Sub
                | MathOp::Mul
                | MathOp::Div
                | MathOp::And
                | MathOp::Or
                | MathOp::Xor
                | MathOp::Nand
                | MathOp::Nor
                | MathOp::Conv
        )
    }
    pub fn unit(self) -> &'static str {
        match self {
            MathOp::Mul => "V\u{b2}",
            MathOp::Div | MathOp::Lg | MathOp::Ln | MathOp::Exp | MathOp::Expr => "",
            MathOp::Intg => "Vs",
            MathOp::Diff => "V/s",
            MathOp::Fft => "dB",
            _ => "V",
        }
    }
    /// Label of the formula, e.g. "CH1 + CH2", "Intg(M1)" (sources as `src_name` indices).
    pub fn formula(self, a: usize, b: usize) -> String {
        let (a, b) = (src_name(a), src_name(b));
        match self {
            MathOp::Add => format!("{} + {}", a, b),
            MathOp::Sub => format!("{} - {}", a, b),
            MathOp::Mul => format!("{} x {}", a, b),
            MathOp::Div => format!("{} / {}", a, b),
            MathOp::And => format!("{} AND {}", a, b),
            MathOp::Or => format!("{} OR {}", a, b),
            MathOp::Xor => format!("{} XOR {}", a, b),
            MathOp::Nand => format!("{} NAND {}", a, b),
            MathOp::Nor => format!("{} NOR {}", a, b),
            MathOp::Conv => format!("Conv({}, {})", a, b),
            op => format!("{}({})", op.name(), a),
        }
    }
    /// Filters with a corner/centre frequency; band filters also have a bandwidth.
    pub fn has_fc(self) -> bool {
        matches!(self, MathOp::Lpf | MathOp::Hpf | MathOp::Bpf | MathOp::Bsf | MathOp::Fir)
    }
    pub fn has_bw(self) -> bool {
        matches!(self, MathOp::Bpf | MathOp::Bsf)
    }
}

/// One math channel's settings.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct MathCh {
    pub op: MathOp,
    pub a: usize,
    pub b: usize,
    pub scale: usize,
    pub fc: f32,
    pub bw: f32,
    pub offset: f32,
    pub win: Window,
    /// A and B of Ax+B
    pub lin: (f32, f32),
}

impl Default for MathCh {
    fn default() -> MathCh {
        MathCh {
            op: MathOp::Off,
            a: 0,
            b: 1,
            scale: 3,
            fc: 10e3,
            bw: 2e3,
            offset: 0.0,
            win: Window::Hann,
            lin: (1.0, 0.0),
        }
    }
}

/// 1-2-5 scale ladder for the math channel: 1 n .. 1 G (units per division).
pub fn math_scales() -> Vec<f64> {
    let mut v = Vec::new();
    let mut d = 1e-9;
    while d < 2e9 {
        for m in [1.0, 2.0, 5.0] {
            v.push(d * m);
        }
        d *= 10.0;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_op_has_a_category() {
        for &op in &MATH_OPS {
            if op == MathOp::Off {
                continue;
            }
            assert!(op.family().ops().contains(&op), "{:?} not listed under {:?}", op, op.family());
        }
        let n: usize = MathFamily::ALL.iter().map(|f| f.ops().len()).sum();
        assert_eq!(n, MATH_OPS.len() - 1);
    }
}
