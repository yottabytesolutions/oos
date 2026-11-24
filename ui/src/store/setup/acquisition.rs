// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) const ACQUISITION: &[Key] = keys! { m, _i, v;
    "active" => value(m.active, p_idx(v, NCH)?);
    "cur_b_source" => custom(
        m.cur_b_source.map(|source| source.token()),
        m.cur_b_source = if v == "none" { None } else { Some(WaveId::parse(v)?) }
    );
    "tdiv" => custom(Some(tscales()[m.tscale].to_string()), m.tscale = tdiv_index(v)?);
    "delay" => value(m.delay, p_f32(v)?);
    "trig_src" => value(m.trig_src, p_idx(v, NCH)?);
    "trig_ref" => custom(
        Some(m.trig_ref.map_or("none".into(), |i| i.to_string())),
        m.trig_ref = if v == "none" { None } else { Some(p_idx(v, NREF)?) }
    );
    "trig_la" => custom(
        Some(m.trig_la.map_or("none".to_string(), |n| n.to_string())),
        m.trig_la = if v.trim() == "none" { None } else { Some(p_idx(v, 16)? as u8) }
    );
    "pat_la" => custom(
        Some(
            m.pat_la
                .iter()
                .map(|&b| match b {
                    Bit::H => 'H',
                    Bit::L => 'L',
                    _ => 'X',
                })
                .collect(),
        ),
        {
            if v.trim().len() != 16 {
                return Err(format!("pat_la: {:?}", v));
            }
            for (b, c) in m.pat_la.iter_mut().zip(v.trim().chars()) {
                *b = match c {
                    'H' => Bit::H,
                    'L' => Bit::L,
                    _ => Bit::X,
                };
            }
        }
    );
    "trig_level" => value(m.trig_level, p_f32(v)?);
    "slope" => named(m.slope, p_enum(v, &SLOPES)?);
    "trig_mode" => named(m.trig_mode, parse_sweep(v)?);
    "holdoff" => value(m.holdoff, p_f32(v)?);
    "trig_type" => named(m.trig_type, p_enum(v, &TRIG_TYPES)?);
    "trig_level2" => value(m.trig_level2, p_f32(v)?);
    "trig_nrej" => value(m.trig_nrej, p_bool(v)?);
    "auto_nrej" => value(m.auto_nrej, p_bool(v)?);
    "trig_coup" => named(m.trig_coup, p_enum(v, &TRIG_COUPS)?);
    "trig_src2" => value(m.trig_src2, p_idx(v, NCH)?);
    "trig_ref2" => custom(
        Some(m.trig_ref2.map_or("none".into(), |i| i.to_string())),
        m.trig_ref2 = if v == "none" { None } else { Some(p_idx(v, NREF)?) }
    );
    "trig_slope2" => named(m.trig_slope2, p_enum(v, &SLOPES)?);
    "trig_pol" => named(m.trig_pol, p_enum(v, &POLS)?);
    "trig_when" => named(m.trig_when, p_enum(v, &WHENS)?);
    "trig_t1" => value(m.trig_t1, p_f32(v)?);
    "trig_t2" => value(m.trig_t2, p_f32(v)?);
    "video_std" => named(m.video_std, p_enum(v, &VIDEO_STDS)?);
    "video_sync" => named(m.video_sync, p_enum(v, &VSYNCS)?);
    "video_line" => value(m.video_line, p_u32(v)?);
    "pat" => custom(
        Some(m.pat.iter().map(|&b| name(b)).collect::<Vec<_>>().join(" ")),
        {
            let b = p_list(v, NCH, |x| p_enum(x, &BITS))?;
            m.pat.copy_from_slice(&b);
        }
    );
    "pat_levels" => custom(
        Some(list(&m.pat_levels)),
        {
            let l = p_list(v, NCH, p_f32)?;
            m.pat_levels.copy_from_slice(&l);
        }
    );
    "win_mode" => named(m.win_mode, p_enum(v, &WIN_MODES)?);
    "nth_idle" => value(m.nth_idle, p_f32(v)?);
    "nth_n" => value(m.nth_n, p_u32(v)?);
    "setup" => value(m.setup, p_f32(v)?);
    "hold" => value(m.hold, p_f32(v)?);
    "acq" => named(m.acq, p_enum(v, &ACQ_MODES)?);
    "cores" => value(m.cores, p_bool(v)?);
    "avg_count" => value(m.avg_count, p_u32(v)?.max(1));
    "avg_hw" => value(m.avg_hw, p_bool(v)?);
    "segments" => value(m.segments, (p_u32(v)? as usize).min(crate::ui::SEG_HW_MAX));
    "seg_hw" => value(m.seg_hw, p_bool(v)?);
    "seg_interval" => value(m.seg_ivl, p_f32(v)?.clamp(0.0, crate::ui::SEG_IVL_MAX));
    "mdepth" => value(m.mdepth, p_idx(v, mdepths().len())?);
    "auto_depth" => custom(
        Some(name(m.auto_acquisition.depth())),
        m.auto_acquisition.set_depth(p_enum(v, &AutoDepth::ALL)?)
    );
    "persist" => value(m.persist, p_idx(v, PERSIST.len())?);
    "intensity" => value(m.intensity, p_u32(v)?.clamp(10, 100));
    "grid" => named(m.grid, p_enum(v, &GRIDS)?);
    "grid_bright" => value(m.grid_bright, p_u32(v)?.clamp(10, 100));
    "grid_frame" => value(m.grid_frame, p_bool(v)?);
    "windows" => named(m.win_layout, p_enum(v, &WinLayout::ALL)?);
    "density" => named(m.density, p_enum(v, &Density::ALL)?);
    "split_pane" => value(m.split_pane, p_idx(v, crate::ui::PANES_N)?);
    "ch_window" => custom(
        Some(m.ch_win.iter().map(|w| (w + 1).to_string()).collect::<Vec<_>>().join(" ")),
        {
            let w = p_list(v, NCH, |x| p_idx(x, 5)?.checked_sub(1).ok_or_else(|| "window 0".to_string()))?;
            m.ch_win.copy_from_slice(&w);
        }
    );
    "math_window" => custom(
        Some(m.math_win.iter().map(|w| (w + 1).to_string()).collect::<Vec<_>>().join(" ")),
        {
            let w = p_list(v, crate::math::NMATH, |x| p_idx(x, 5)?.checked_sub(1).ok_or_else(|| "window 0".to_string()))?;
            m.math_win.copy_from_slice(&w);
        }
    );
    "la_window" => custom(Some((m.la_win + 1).to_string()), m.la_win = p_idx(v, 5)?.checked_sub(1).ok_or("window 0")?);
    "ref_window" => custom(
        Some(m.ref_win.iter().map(|w| (w + 1).to_string()).collect::<Vec<_>>().join(" ")),
        {
            let w = p_list(v, crate::model::NREF, |x| p_idx(x, 5)?.checked_sub(1).ok_or_else(|| "window 0".to_string()))?;
            m.ref_win.copy_from_slice(&w);
        }
    );
    "meas" => custom(
        Some(m.meas.iter().map(|&(k, id)| format!("{:?}:{}", k, id.token())).collect::<Vec<_>>().join(" ")),
        {
            let mut out = Vec::new();
            for e in v.split_whitespace() {
                let (k, c) = e.split_once(':').ok_or_else(|| format!("not item:channel: {}", e))?;
                out.push((p_enum(k, &ALL_MEAS)?, WaveId::parse(c)?));
            }
            if out.len() + m.meas2.len() + m.cat.len() > MEAS_SELECTION_MAX {
                return Err("too many selected measurements".into());
            }
            m.meas = out;
        }
    );
    "meas2" => custom(
        Some(
            m.meas2.iter().map(|&(k, a, b)| format!("{}:{}:{}", k, a.token(), b.token())).collect::<Vec<_>>().join(" "),
        ),
        {
            let mut out = Vec::new();
            for e in v.split_whitespace() {
                let f: Vec<&str> = e.split(':').collect();
                let [k, a, b] = f[..] else { return Err(format!("not item:a:b: {}", e)) };
                out.push((p_idx(k, crate::ui::PAIRS_N)?, WaveId::parse(a)?, WaveId::parse(b)?));
            }
            if m.meas.len() + out.len() + m.cat.len() > MEAS_SELECTION_MAX {
                return Err("too many selected measurements".into());
            }
            m.meas2 = out;
        }
    );
    "cat" => custom(
        (!m.cat.is_empty()).then(|| {
            m.cat
                .iter()
                .map(|&(k, a, b, c, d)| format!(
                    "{}:{}:{}:{}:{}",
                    k, a.token(), b.token(), c.token(), d.token()
                ))
                .collect::<Vec<_>>()
                .join(" ")
        }),
        {
            let mut out = Vec::new();
            for e in v.split_whitespace() {
                let f: Vec<&str> = e.split(':').collect();
                let [k, a, b, c, d] = f[..] else { return Err(format!("not item:a:b:c:d: {}", e)) };
                let id = k.parse::<u16>().map_err(|_| format!("not a catalog id: {}", k))?;
                if !catalog_id_valid(id) {
                    return Err(format!("unknown or non-Wave catalog id: {id}"));
                }
                if m.meas.len() + m.meas2.len() + out.len() >= MEAS_SELECTION_MAX {
                    return Err("too many selected measurements".into());
                }
                let row = (id, WaveId::parse(a)?, WaveId::parse(b)?, WaveId::parse(c)?, WaveId::parse(d)?);
                if out.contains(&row) {
                    return Err("duplicate catalog row".into());
                }
                out.push(row);
            }
            m.cat = out;
        }
    );
    "dvm" => custom(
        Some(format!("{} {} {}", m.dvm_on, m.dvm_src, m.dvm_mode)),
        {
            let f: Vec<&str> = v.split_whitespace().collect();
            let [on, c, mode] = f[..] else { return Err("dvm: on source mode".into()) };
            (m.dvm_on, m.dvm_src, m.dvm_mode) = (p_bool(on)?, p_idx(c, NCH)?, p_idx(mode, 3)?);
        }
    );
    "counter" => custom(
        Some(format!(
            "{} {} {} {}",
            m.cnt_on, m.cnt_src, m.cnt_mode, m.cnt_digits
        )),
        {
            let f: Vec<&str> = v.split_whitespace().collect();
            let [on, c, mode, d] = f[..] else { return Err("counter: on source mode digits".into()) };
            (m.cnt_on, m.cnt_src, m.cnt_mode) = (p_bool(on)?, p_idx(c, NCH)?, p_idx(mode, 3)?);
            m.cnt_digits = p_idx(d, 8)?.max(3) as u32;
        }
    );
};
