// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) const CHANNEL: &[Key] = keys! { m, _i, v;
    "on" => value(m.ch[_i].on, p_bool(v)?);
    "vdiv" => custom(
        Some(VSCALES[m.ch[_i].vscale].to_string()),
        {
            let x = p_f32(v)?;
            // 20 V/div was once on the ladder, the hardware ends at 10
            let x = x.min(VSCALES[VSCALES.len() - 1]);
            m.ch[_i].vscale =
                VSCALES.iter().position(|&s| (s - x).abs() <= s * 1e-3).ok_or_else(|| format!("no V/div step {}", v))?;
        }
    );
    "vfine" => custom(
        (m.ch[_i].vfine != 1.0).then(|| m.ch[_i].vfine.to_string()),
        m.ch[_i].vfine = p_f32(v)?.clamp(1.0, 2.5)
    );
    "unit" => custom(
        (m.ch[_i].unit != AmpUnit::V).then(|| m.ch[_i].unit.name().to_string()),
        m.ch[_i].unit = *AmpUnit::ALL.iter().find(|u| u.name() == v.trim()).ok_or_else(|| format!("no unit {}", v))?
    );
    "bias" => custom((m.ch[_i].bias != 0.0).then(|| m.ch[_i].bias.to_string()), m.ch[_i].bias = p_f32(v)?);
    "skew" => custom(
        (m.ch[_i].skew != 0.0).then(|| m.ch[_i].skew.to_string()),
        m.ch[_i].skew = p_f32(v)?.clamp(-SKEW_MAX, SKEW_MAX)
    );
    "offset" => value(m.ch[_i].offset, p_f32(v)?);
    "coupling" => named(m.ch[_i].coupling, p_enum(v, &COUPLINGS)?);
    "bw" => custom(
        Some(BWS[m.ch[_i].bw].0.to_string()),
        m.ch[_i].bw = BWS.iter().position(|b| b.0 == v.trim()).ok_or_else(|| format!("unknown bandwidth {}", v))?
    );
    "probe" => value(m.ch[_i].probe, p_f32(v)?);
    "invert" => value(m.ch[_i].invert, p_bool(v)?);
    "vscale" => custom(None, m.ch[_i].vscale = (p_idx(v, VSCALES.len() - 1)? + 2).min(VSCALES.len() - 1));
    "bw20" => custom(None, m.ch[_i].bw = if p_bool(v)? { BW_20M } else { BW_FULL });
};
pub(super) const LABEL: &[Key] = keys! { m, _i, v;
    "label" => custom(
        (!m.ch_label[_i].is_empty()).then(|| m.ch_label[_i].clone()),
        m.ch_label[_i] = v.trim().chars().take(10).collect()
    );
};

pub(super) const DESKEW_CH: &[Key] = keys! { m, _i, v;
    "" => custom(
        (m.deskew_ch[_i] != 0.0).then(|| m.deskew_ch[_i].to_string()),
        {
            let value =
                v.parse::<f64>().ok().filter(|x| x.is_finite() && x.abs() <= 1000.0).ok_or("invalid deskew seconds")?;
            m.deskew_ch[_i] = value;
        }
    );
};
pub(super) const DESKEW_REF: &[Key] = keys! { m, _i, v;
    "" => custom(
        (m.deskew_ref[_i] != 0.0).then(|| m.deskew_ref[_i].to_string()),
        {
            let value =
                v.parse::<f64>().ok().filter(|x| x.is_finite() && x.abs() <= 1000.0).ok_or("invalid deskew seconds")?;
            m.deskew_ref[_i] = value;
        }
    );
};
