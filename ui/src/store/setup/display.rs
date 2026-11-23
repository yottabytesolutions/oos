// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) const CURSOR_KEYS: &[Key] = keys! { m, _i, v;
    "cursor" => named(m.cursor, p_enum(v, &CURSORS)?);
    "cur_a" => value(m.cur_a, p_f32(v)?);
    "cur_b" => value(m.cur_b, p_f32(v)?);
    "cur_c" => value(m.cur_c, p_f32(v)?);
    "cur_d" => value(m.cur_d, p_f32(v)?);
    "touch_lock" => value(m.touch_lock, p_bool(v)?);
    "aux_out" => named(m.aux_out, p_enum(v, &AUX_OUTS)?);
    "fine_v" => value(m.fine_v, p_bool(v)?);
    "fine_t" => value(m.fine_t, p_bool(v)?);
    "tfine" => custom((m.tfine != 1.0).then(|| m.tfine.to_string()), m.tfine = p_f32(v)?.clamp(0.1, 1.0));
    "v_expand_gnd" => value(m.v_expand_gnd, p_bool(v)?);
    "h_expand" => custom(Some(format!("{:?}", m.h_expand)), m.h_expand = p_enum(v, &HEXPANDS)?);
    "math" => named(m.math, p_enum(v, &MATH_OPS)?);
    "math_a" => value(m.math_a, p_idx(v, crate::math::MATH_SOURCES)?);
    "math_b" => value(m.math_b, p_idx(v, crate::math::MATH_SOURCES)?);
    "math_scale" => value(m.math_scale, p_idx(v, crate::math::math_scales().len())?);
    "math_fc" => value(m.math_fc, p_f32(v)?);
    "math_lin" => custom(
        Some(format!("{} {}", m.math_lin.0, m.math_lin.1)),
        {
            let (a, b) = v.split_once(' ').ok_or("math_lin: two numbers")?;
            m.math_lin = (p_f32(a)?, p_f32(b)?);
        }
    );
};
pub(super) const DISPLAY: &[Key] = keys! { m, _i, v;
    "pane" => value(m.pane, p_idx(v, crate::ui::PANES_N)?);
    "meas_labels" => value(m.meas_labels, p_bool(v)?);
    "strip_dock" => named(m.strip_dock, p_enum(v, &StripDock::ALL)?);
    "trend_sel" => custom(
        Some(format!("{} {}", m.trend_sel[0], m.trend_sel[1])),
        {
            let f: Vec<&str> = v.split_whitespace().collect();
            let [a, b] = f[..] else { return Err("trend_sel: two values".into()) };
            m.trend_sel = [p_idx(a, crate::ui::TREND_SEL_N)?, p_idx(b, crate::ui::TREND_SEL_N)?];
        }
    );
    "zoom" => value(m.zoom, p_bool(v)?);
    "auto_roll" => value(m.auto_roll, p_bool(v)?);
    "zoom_tdiv" => custom(Some(tscales()[m.zscale].to_string()), m.zscale = tdiv_index(v)?);
    "zdelay" => value(m.zdelay, p_f32(v)?);
    "color_grade" => value(m.color_grade, p_bool(v)?);
    "dots" => custom(
        Some(name(m.dots)),
        {
            m.dots = match v {
                "true" | "false" => DotsMode::Auto,
                _ => p_enum(v, &DOTS_MODES)?,
            };
        }
    );
    "bar" => custom(
        Some(if m.bar_compact { "compact" } else { "cards" }.to_string()),
        {
            m.bar_compact = match v.trim() {
                "compact" => true,
                "cards" => false,
                _ => return Err(format!("{:?}: cards or compact", v)),
            };
        }
    );
    "hdmi" => named(m.hdmi, p_enum(v, &HDMI_MODES)?);
    "hdmi_view" => custom(
        Some(m.hdmi_view.to_string()),
        {
            let n = v.trim().parse::<usize>().map_err(|e| e.to_string())?;
            if n > crate::ui::VIEWS_MAX {
                return Err(format!("{:?}: 0 (the panel's) to {}", v, crate::ui::VIEWS_MAX));
            }
            m.hdmi_view = n;
        }
    );
    "view" => named(m.view, p_enum(v, &crate::ui::stored_views())?);
    "views" => custom(
        Some(format!("{} {}", m.views, m.view_cur + 1)),
        {
            let n = p_list(v, 2, |x| x.parse::<usize>().map_err(|e| e.to_string()))?;
            if !(1..=crate::ui::VIEWS_MAX).contains(&n[0]) || !(1..=n[0]).contains(&n[1]) {
                return Err(format!("{:?}: count 1 to {}, then the one shown", v, crate::ui::VIEWS_MAX));
            }
            (m.views, m.view_cur) = (n[0], n[1] - 1);
        }
    );
};

pub(super) const VIEWS: &[Key] = keys! { m, _i, v;
    "" => custom(
        (!m.view_slots[_i].is_empty()).then(|| m.view_slots[_i].clone()),
        m.view_slots[_i] = crate::ui::views::slot_checked(v)?
    );
};
