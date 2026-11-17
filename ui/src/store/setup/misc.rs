// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) const MISC: &[Key] = keys! { m, _i, v;
    "z_src" => custom(
        Some(m.z_src.map_or("off".to_string(), |c| (c + 1).to_string())),
        m.z_src = if v.trim() == "off" { None } else { Some(p_idx(v, NCH + 1)?.max(1) - 1) }
    );
    "hist" => named(m.hist, p_enum(v, &[Hist::Off, Hist::Vertical, Hist::Horizontal])?);
    "mask_on" => value(m.mask_on, p_bool(v)?);
    "mask_src" => value(m.mask_src, p_idx(v, NCH)?);
    "mask_x" => value(m.mask_x, p_f32(v)?);
    "mask_y" => value(m.mask_y, p_f32(v)?);
    "mask_stop" => value(m.mask_stop, p_bool(v)?);
    "mask_inv" => value(m.mask_inv, p_bool(v)?);
    "quick" => named(m.quick, p_enum(v, &QUICKS)?);
    "text_size" => named(m.text_size, p_enum(v, &TEXT_SIZES)?);
    "la" => custom(Some(m.la.to_text()), m.la = crate::ui::la::LaSet::parse(v)?);
    "dec_la" => custom(
        Some(format!(
            "{} {}",
            m.dec_la_in as u8,
            m.dec_la.map(|n| n.map_or("-".to_string(), |x| x.to_string())).join(" ")
        )),
        {
            let w: Vec<&str> = v.split_whitespace().collect();
            if w.len() != 5 {
                return Err(format!("dec_la: {:?}", v));
            }
            m.dec_la_in = w[0] == "1";
            for (r, s) in m.dec_la.iter_mut().zip(&w[1..]) {
                *r = if *s == "-" { None } else { Some(s.parse::<u8>().map_err(|_| format!("dec_la: {:?}", v))?.min(15)) };
            }
        }
    );
};
pub(super) const LEGACY: &[Key] = keys! { m, _i, v;
    "cur_b_ch" => custom(None, m.cur_b_source = Some(WaveId::Ch(p_idx(v, NCH)?)));
    "tscale" => custom(None, m.tscale = legacy_tscale(v, tscales().len())?);
    "zscale" => custom(None, m.zscale = legacy_tscale(v, tscales().len())?);
    "hdmi_wave" => custom(None, m.hdmi = if p_bool(v)? { HdmiMode::Waveform } else { HdmiMode::Mirror });
};

pub(super) const ZONES: &[Key] = keys! { m, _i, v;
    "" => custom(
        {
            let z = &m.zones[_i];
            let pts: Vec<String> = z.pts.iter().map(|(t, v)| format!("{}:{}", t, v)).collect();
            Some(format!(
                "{} {} {} {}",
                z.on, z.src + 1, z.inside, pts.join(" ")
            ).trim().to_string())
        },
        {
            let z = &mut m.zones[_i];
            let w: Vec<&str> = v.split_whitespace().collect();
            if w.len() < 3 {
                return Err("zone: want on src inside points".into());
            }
            z.on = p_bool(w[0])?;
            z.src = p_idx(w[1], NCH + 1)?.max(1) - 1;
            z.inside = p_bool(w[2])?;
            z.pts = w[3..]
                .iter()
                .map(|p| {
                    let (t, v) = p.split_once(':').ok_or("zone point: want t:v")?;
                    Ok((t.parse::<f64>().map_err(|e| e.to_string())?, v.parse::<f64>().map_err(|e| e.to_string())?))
                })
                .collect::<Result<_, String>>()?;
        }
    );
};
