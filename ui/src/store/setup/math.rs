// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) const MATH: &[Key] = keys! { m, _i, v;
    "log_ivl" => value(m.log_ivl, p_idx(v, crate::ui::LOG_STEPS_N)?);
    "spec_margin" => value(m.spec_margin, p_f32(v)?.clamp(0.0, 60.0));
    "math_offset" => value(m.math_offset, p_f32(v)?);
    "fft_win" => named(m.fft_win, p_enum(v, &WINDOWS)?);
    "math_bw" => value(m.math_bw, p_f32(v)?);
    "math_sel" => value(m.math_sel, p_idx(v, crate::math::NMATH)?);
};
pub(super) const MATH_CHANNEL: &[Key] = keys! { m, _i, v;
    "op" => named(m.maths[_i].op, p_enum(v, &MATH_OPS)?);
    "a" => value(m.maths[_i].a, p_idx(v, crate::math::MATH_SOURCES)?);
    "b" => value(m.maths[_i].b, p_idx(v, crate::math::MATH_SOURCES)?);
    "scale" => value(m.maths[_i].scale, p_idx(v, crate::math::math_scales().len())?);
    "fc" => value(m.maths[_i].fc, p_f32(v)?);
    "bw" => value(m.maths[_i].bw, p_f32(v)?);
    "offset" => value(m.maths[_i].offset, p_f32(v)?);
    "win" => named(m.maths[_i].win, p_enum(v, &WINDOWS)?);
    "lin" => custom(
        Some(format!("{} {}", m.maths[_i].lin.0, m.maths[_i].lin.1)),
        {
            let (a, b) = v.split_once(' ').ok_or("lin: two numbers")?;
            m.maths[_i].lin = (p_f32(a)?, p_f32(b)?);
        }
    );
};

pub(super) const EXPRESSIONS: &[Key] = keys! { m, _i, v;
    "" => custom(
        (!m.math_expr[_i].is_empty()).then(|| m.math_expr[_i].clone()),
        m.math_expr[_i] = v.trim().chars().take(oos_dsp::expr::EXPR_MAX).collect()
    );
};
