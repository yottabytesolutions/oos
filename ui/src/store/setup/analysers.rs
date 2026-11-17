// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

pub(super) const ANALYSERS: &[Key] = keys! { m, _i, v;
    "bode" => custom(
        Some(format!(
            "{} {} {} {} {} {} {} {}",
            m.bode_start, m.bode_stop, m.bode_points, m.bode_amp, m.bode_in.token(), m.bode_out.token(), m.bode_gen,
            m.bode_addr
        )),
        {
            // the address is the rest of the line: it may be empty
            let w: Vec<&str> = v.trim().splitn(8, char::is_whitespace).collect();
            if w.len() < 7 {
                return Err("bode: want 8 values".into());
            }
            let f = |s: &str| s.parse::<f64>().map_err(|e| e.to_string());
            (m.bode_start, m.bode_stop, m.bode_amp) = (f(w[0])?, f(w[1])?, f(w[3])?);
            m.bode_points = p_u32(w[2])?.clamp(2, 1000) as usize;
            m.bode_in = WaveId::parse(w[4])?;
            m.bode_out = WaveId::parse(w[5])?;
            m.bode_gen = p_idx(w[6], crate::ui::bode::GENERATORS.len())?;
            m.bode_addr = w.get(7).map_or("", |a| a.trim()).to_string();
        }
    );
    "bode_link" => custom(
        Some(if m.bode_usb { "usb" } else { "lan" }.to_string()),
        {
            m.bode_usb = match v.trim() {
                "usb" => true,
                "lan" => false,
                _ => return Err(format!("not lan or usb: {}", v)),
            };
        }
    );
    "bode_sweep" => custom(
        Some(format!(
            "{} {} {} {} {} {} {}",
            m.bode_sw_time, m.bode_sw_log, m.bode_sync + 1, name(m.bode_sync_slope), m.bode_sync_level, m.bode_sw_in,
            m.bode_ref
        )),
        {
            let w: Vec<&str> = v.split_whitespace().collect();
            if w.len() != 7 {
                return Err("bode_sweep: want 7 values".into());
            }
            let f = |s: &str| s.parse::<f64>().map_err(|e| e.to_string());
            m.bode_sw_time = f(w[0])?;
            if !m.bode_sw_time.is_finite() || m.bode_sw_time <= 0.0 {
                return Err("bode_sweep: the sweep time must be above 0".into());
            }
            m.bode_sw_log = p_bool(w[1])?;
            m.bode_sync = p_idx(w[2], NCH + 1)?.max(1) - 1;
            m.bode_sync_slope = p_enum(w[3], &SLOPES[..2])?;
            m.bode_sync_level = p_f32(w[4])?;
            m.bode_sw_in = p_idx(w[5], NCH + 1)?;
            m.bode_ref = f(w[6])?;
        }
    );
    "spec" => custom(
        Some(format!(
            "{} {} {} {} {} {} {} {} {}",
            m.spec_src.token(), m.spec_center, m.spec_span, m.spec_rbw, name(m.spec_win), m.spec_trace, m.spec_ref,
            m.spec_wf, m.spec_dist
        )),
        {
            let w: Vec<&str> = v.split_whitespace().collect();
            if w.len() != 9 {
                return Err("spec: want 9 values".into());
            }
            let f = |s: &str| s.parse::<f64>().map_err(|e| e.to_string());
            m.spec_src = WaveId::parse(w[0])?;
            (m.spec_center, m.spec_span, m.spec_rbw) = (f(w[1])?, f(w[2])?, f(w[3])?);
            m.spec_win = p_enum(w[4], &WINDOWS)?;
            m.spec_trace = p_idx(w[5], 4)?;
            m.spec_ref = p_f32(w[6])?;
            (m.spec_wf, m.spec_dist) = (p_bool(w[7])?, p_bool(w[8])?);
        }
    );
    "spec_peaks" => custom(
        Some(format!("{} {}", m.spec_sort, m.spec_harm)),
        {
            let f: Vec<&str> = v.split_whitespace().collect();
            let [sort, harm] = f[..] else { return Err("spec_peaks: order harmonics".into()) };
            (m.spec_sort, m.spec_harm) = (p_idx(sort, 2)?, p_idx(harm, crate::ui::PANE_HARMS.len())?);
        }
    );
    "eye" => custom(
        Some(format!("{} {} {}", m.eye_src.token(), m.eye_rate, if m.eye_from_record { 1 } else { 0 })),
        {
            let w: Vec<&str> = v.split_whitespace().collect();
            m.eye_src = WaveId::parse(w.first().ok_or("eye: source")?)?;
            m.eye_rate = w.get(1).ok_or("eye: rate")?.parse::<f64>().map_err(|e| e.to_string())?;
            // older files have no third field: the rate was entered, not measured
            m.eye_from_record = w.get(2).is_some_and(|s| *s != "0");
        }
    );
    "eye_place" => named(m.eye_place, p_enum(v, &[EyePlace::Full, EyePlace::Side, EyePlace::Under])?);
    "power" => custom(
        Some(format!("{} {} {}", m.pw_v.token(), m.pw_i.token(), name(m.pw_mode))),
        {
            let w: Vec<&str> = v.split_whitespace().collect();
            if w.len() != 3 {
                return Err("power: want 3 values".into());
            }
            m.pw_v = WaveId::parse(w[0])?;
            m.pw_i = WaveId::parse(w[1])?;
            m.pw_mode = p_enum(
                w[2],
                &[
                    PowerMode::Quality,
                    PowerMode::Harmonics,
                    PowerMode::Switching,
                    PowerMode::Ripple,
                    PowerMode::Capacitance,
                    PowerMode::Rail,
                ],
            )?;
        }
    );
    "meas_cfg" => custom(
        Some(format!(
            "{} {:?} {} {} {} {} {} {}",
            m.meas_idle, m.meas_out, m.meas_n, if m.meas_data { 1 } else { 0 }, m.meas_lpct, m.meas_hpct,
            m.meas_tsetup, m.meas_thold
        )),
        {
            let w: Vec<&str> = v.split_whitespace().collect();
            if w.len() != 4 && w.len() != 8 {
                return Err("meas_cfg: idle outside n data [lpct hpct tsetup thold]".into());
            }
            m.meas_idle = w[0].parse::<f64>().map_err(|e| e.to_string())?;
            m.meas_out = p_enum(
                w[1],
                &[oos_dsp::measure::Outside::Above, oos_dsp::measure::Outside::Below, oos_dsp::measure::Outside::Both],
            )?;
            m.meas_n = p_u32(w[2])?.clamp(1, 32);
            m.meas_data = p_bool(w[3])?;
            if w.len() == 8 {
                m.meas_lpct = w[4].parse::<f64>().map_err(|e| e.to_string())?;
                m.meas_hpct = w[5].parse::<f64>().map_err(|e| e.to_string())?;
                m.meas_tsetup = w[6].parse::<f64>().map_err(|e| e.to_string())?;
                m.meas_thold = w[7].parse::<f64>().map_err(|e| e.to_string())?;
            };
        }
    );
    "meas_full_record" => value(m.meas_full_record, p_bool(v)?);
    "spec_acpr" => custom(
        Some(format!("{} {}", m.spec_acpr_w, m.spec_acpr_s)),
        {
            let w: Vec<&str> = v.split_whitespace().collect();
            let [width, space] = w[..] else { return Err("spec_acpr: width spacing".into()) };
            m.spec_acpr_w = width.parse::<f64>().map_err(|e| e.to_string())?;
            m.spec_acpr_s = space.parse::<f64>().map_err(|e| e.to_string())?;
        }
    );
    "bode_mode" => named(
        m.bode_mode,
        p_enum(v, &[oos_dsp::bode::Mode::Loop, oos_dsp::bode::Mode::Psrr, oos_dsp::bode::Mode::Z])?
    );
    "eye_more" => custom(
        Some(format!(
            "{} {} {} {} {} {} {} {}",
            m.eye_ber, m.eye_pn_lo, m.eye_pn_hi, if m.eye_npj { 1 } else { 0 }, opt_wave_dash(m.eye_rail), m.eye_notch,
            opt_wave_dash(m.eye_b), if m.eye_hp { 1 } else { 0 }
        )),
        {
            let w: Vec<&str> = v.split_whitespace().collect();
            let [ber, lo, hi, npj, rail, notch, b, hp] = w[..] else { return Err("eye_more".into()) };
            let opt = |s: &str| if s == "-" { Ok(None) } else { WaveId::parse(s).map(Some) };
            m.eye_ber = ber.parse::<f64>().map_err(|e| e.to_string())?;
            m.eye_pn_lo = lo.parse::<f64>().map_err(|e| e.to_string())?;
            m.eye_pn_hi = hi.parse::<f64>().map_err(|e| e.to_string())?;
            m.eye_npj = p_bool(npj)?;
            m.eye_rail = opt(rail)?;
            m.eye_notch = notch.parse::<f64>().map_err(|e| e.to_string())?;
            m.eye_b = opt(b)?;
            m.eye_hp = p_bool(hp)?;
        }
    );
    "power_more" => custom(
        Some(format!(
            "{} {} {} {} {}",
            m.pw_fline, m.pw_fsw, opt_wave_dash(m.pw_vin),
            m.pw_ref.map(|v| v.to_string()).unwrap_or_else(|| "-".into()), opt_wave_dash(m.pw_clk)
        )),
        {
            let w: Vec<&str> = v.split_whitespace().collect();
            let [fline, fsw, vin, reference, clk] = w[..] else { return Err("power_more".into()) };
            let opt = |s: &str| if s == "-" { Ok(None) } else { WaveId::parse(s).map(Some) };
            m.pw_fline = fline.parse::<f64>().map_err(|e| e.to_string())?;
            m.pw_fsw = fsw.parse::<f64>().map_err(|e| e.to_string())?;
            m.pw_vin = opt(vin)?;
            m.pw_ref = if reference == "-" { None } else { Some(reference.parse::<f64>().map_err(|e| e.to_string())?) };
            m.pw_clk = opt(clk)?;
        }
    );
};
