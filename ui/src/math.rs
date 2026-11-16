// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Math rendering bridge and computation. Settings live in oos-core.

pub use oos_core::math::*;
use oos_dsp::linecode::{self, Gate};
use oos_dsp::math::{binary, unary, Op1, Op2};

/// The math waveform (not for FFT/Off); `fc` = filter corner/centre frequency, `bw` = band filter bandwidth.
pub fn compute(op: MathOp, a: &[f32], b: &[f32], dt: f64, fc: f64, bw: f64) -> Vec<f32> {
    match op {
        MathOp::Add => binary(Op2::Add, a, b),
        MathOp::Sub => binary(Op2::Sub, a, b),
        MathOp::Mul => binary(Op2::Mul, a, b),
        MathOp::Div => binary(Op2::Div, a, b),
        MathOp::Intg => unary(Op1::Intg, a, dt),
        MathOp::Diff => unary(Op1::Diff, a, dt),
        MathOp::Abs => unary(Op1::Abs, a, dt),
        MathOp::Sqrt => unary(Op1::Sqrt, a, dt),
        MathOp::Lg => unary(Op1::Lg, a, dt),
        MathOp::Ln => unary(Op1::Ln, a, dt),
        // applied by the caller, which knows A and B
        MathOp::Lin | MathOp::Expr => a.to_vec(),
        MathOp::Exp => unary(Op1::Exp, a, dt),
        MathOp::Lpf => unary(Op1::Lowpass(fc), a, dt),
        MathOp::Hpf => unary(Op1::Highpass(fc), a, dt),
        MathOp::Bpf => unary(Op1::Bandpass(fc, bw), a, dt),
        MathOp::Bsf => unary(Op1::Bandstop(fc, bw), a, dt),
        MathOp::Off | MathOp::Fft => Vec::new(),
        MathOp::Fir => linecode::fir_lowpass(a, dt, fc),
        MathOp::And => linecode::gate(a, b, Gate::And),
        MathOp::Or => linecode::gate(a, b, Gate::Or),
        MathOp::Xor => linecode::gate(a, b, Gate::Xor),
        MathOp::Nand => linecode::gate(a, b, Gate::Nand),
        MathOp::Nor => linecode::gate(a, b, Gate::Nor),
        MathOp::Not => linecode::not(a),
        MathOp::A2d => linecode::a2d(a),
        MathOp::D2a => linecode::d2a(a),
        MathOp::Conv => linecode::conv(a, b),
        MathOp::Unnrzi => linecode::unnrzi(a),
    }
}

/// Math sources: channels, math channels, then the refs (`MATH_REF_BASE`).
pub const MATH_SOURCES: usize = MATH_REF_BASE + crate::model::NREF;

pub fn parse_expr(text: &str) -> Result<oos_dsp::expr::Expr, String> {
    let names: Vec<_> = (0..MATH_SOURCES).map(src_name).collect();
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    oos_dsp::expr::Expr::parse_sources(text, &names)
}
