// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
pub(super) const CASES: &[Case] = &[
    (
        "math",
        |m| {
            m.math =
                if m.math == crate::math::MathOp::Off { crate::math::MathOp::Add } else { crate::math::MathOp::Off };
        },
        C::MATH.union(C::STATS),
    ),
    (
        "math_a",
        |m| {
            m.math_a += 1;
        },
        C::MATH.union(C::STATS),
    ),
    (
        "math_b",
        |m| {
            m.math_b += 1;
        },
        C::MATH.union(C::STATS),
    ),
    (
        "math_scale",
        |m| {
            m.math_scale += 1;
        },
        C::MATH.union(C::STATS),
    ),
    (
        "math_fc",
        |m| {
            m.math_fc += 1.0;
        },
        C::MATH.union(C::STATS),
    ),
    (
        "math_lin",
        |m| {
            m.math_lin.0 += 1.0;
        },
        C::MATH.union(C::STATS),
    ),
    (
        "math_expr",
        |m| {
            m.math_expr[0].push_str("fixture");
        },
        C::MATH.union(C::STATS),
    ),
    (
        "spec_margin",
        |m| {
            m.spec_margin += 1.0;
        },
        C::MATH.union(C::VIEW),
    ),
    (
        "math_offset",
        |m| {
            m.math_offset += 1.0;
        },
        C::MATH.union(C::STATS),
    ),
    (
        "fft_win",
        |m| {
            m.fft_win = if m.fft_win == crate::math::Window::Rect {
                crate::math::Window::Hann
            } else {
                crate::math::Window::Rect
            };
        },
        C::MATH.union(C::STATS),
    ),
    (
        "math_bw",
        |m| {
            m.math_bw += 1.0;
        },
        C::MATH.union(C::STATS),
    ),
    (
        "math_sel",
        |m| {
            m.math_sel += 1;
        },
        C::NONE,
    ),
    (
        "maths",
        |m| {
            m.maths[0].fc += 1.0;
        },
        C::MATH.union(C::STATS),
    ),
    (
        "decs",
        |m| {
            m.decs[0] = Some(m.dec_cfg());
        },
        C::NONE,
    ),
    (
        "dec_sel",
        |m| {
            m.dec_sel += 1;
        },
        C::NONE,
    ),
    (
        "dec_on",
        |m| {
            m.dec_on = !m.dec_on;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_src",
        |m| {
            m.dec_src += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_baud",
        |m| {
            m.dec_baud += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_canrate",
        |m| {
            m.dec_canrate += 1;
        },
        C::ACQ.union(C::STATS),
    ),
    (
        "dec_canh",
        |m| {
            m.dec_canh = !m.dec_canh;
        },
        C::ACQ.union(C::STATS),
    ),
];
