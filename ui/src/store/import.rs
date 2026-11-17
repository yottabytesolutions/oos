// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::{
    cstr, endian_f32, endian_f64, endian_u16, endian_u32, looks_ag, WaveTrace, MAX_WAVEFORM_BYTES, MAX_WAVEFORM_SAMPLES,
};

/// Tek reference waveform version 001/002/003, YT vector, first frame.
/// Version 001 uses an 88-byte reference block, 156-byte explicit dimensions,
/// and 132-byte implicit dimensions. Later versions use 90/160/136 bytes.
/// See Tektronix Reference Waveform File Format (077-0220-10/11).
pub fn parse_wfm(b: &[u8]) -> Result<WaveTrace, String> {
    if b.len() < 78 {
        return Err("truncated WFM".into());
    }
    let le = match endian_u16(b, 0, true) {
        0x0F0F => true,
        0xF0F0 => false,
        _ => return Err("header is not a Tek WFM version 001, 002 or 003".into()),
    };
    let ver = &b[2..10];
    if ver != b":WFM#001" && ver != b":WFM#002" && ver != b":WFM#003" {
        return Err("header is not a Tek WFM version 001, 002 or 003".into());
    }
    let bpp = b[15] as usize;
    if bpp == 0 || bpp > 8 {
        return Err("WFM sample size".into());
    }
    let curve = usize::try_from(endian_u32(b, 16, le) as i32).map_err(|_| "WFM curve offset is negative")?;
    let label = b[40..72].iter().take_while(|&&c| c != 0).map(|&c| c as char).collect::<String>().trim().to_string();
    let (i, vscale, voff, fmt, dt, t0) = wfm_dimensions(b, le, ver)?;
    let data_start = endian_u32(b, i + 14, le) as usize;
    let post_start = endian_u32(b, i + 18, le) as usize;
    if !dt.is_finite() || dt <= 0.0 || !t0.is_finite() || !vscale.is_finite() || !voff.is_finite() {
        return Err("WFM time and voltage scaling must be finite with a positive time step".into());
    }
    if curve < i + 30 || data_start > post_start {
        return Err("WFM curve is empty".into());
    }
    let expected_bpp = match fmt {
        0 => 2,
        1 | 2 | 4 => 4,
        3 | 5 => 8,
        6 | 7 => 1,
        _ => return Err("WFM sample format".into()),
    };
    if bpp != expected_bpp {
        return Err("WFM sample format does not match bytes per point".into());
    }
    let start = curve.checked_add(data_start).ok_or("WFM curve offset overflow")?;
    let stop = curve.checked_add(post_start).ok_or("WFM curve offset overflow")?;
    if stop > b.len().saturating_sub(8) {
        return Err("truncated WFM curve".into());
    }
    if stop <= start || !(stop - start).is_multiple_of(bpp) {
        return Err("WFM curve is empty".into());
    }
    let n = (stop - start) / bpp;
    if n < 2 {
        return Err("WFM curve is empty".into());
    }
    if n > MAX_WAVEFORM_SAMPLES {
        return Err("waveform exceeds the 32-million-sample import limit".into());
    }
    let v = wfm_samples(&b[start..stop], le, fmt, bpp, vscale, voff)?;
    Ok(WaveTrace { t0, dt, v, label })
}

fn wfm_dimensions(b: &[u8], le: bool, ver: &[u8]) -> Result<(usize, f64, f64, u32, f64, f64), String> {
    let (reference_size, explicit_size, implicit_size) =
        if ver == b":WFM#001" { (88, 156, 132) } else { (90, 160, 136) };
    // Packed header follows the common 78-byte static block.
    let mut i = 78usize;
    let need = |n: usize| -> Result<(), String> {
        if b.len() < n {
            Err("truncated WFM".into())
        } else {
            Ok(())
        }
    };
    need(i + reference_size)?;
    i += reference_size;
    need(i + explicit_size)?;
    let vscale = endian_f64(b, i, le);
    let voff = endian_f64(b, i + 8, le);
    let fmt = endian_u32(b, i + 72, le); // after scale, offset, size, units, 4 doubles
    i += explicit_size;
    need(i + explicit_size)?;
    i += explicit_size;
    need(i + implicit_size)?;
    let dt = endian_f64(b, i, le);
    let t0 = endian_f64(b, i + 8, le);
    i += implicit_size;
    need(i + implicit_size + 12 + 12 + 24 + 30)?;
    i += implicit_size + 12 + 12 + 24;
    Ok((i, vscale, voff, fmt, dt, t0))
}

fn wfm_samples(b: &[u8], le: bool, fmt: u32, bpp: usize, vscale: f64, voff: f64) -> Result<Vec<f32>, String> {
    let mut v = Vec::with_capacity(b.len() / bpp);
    for k in 0..b.len() / bpp {
        let o = k * bpp;
        let code = match fmt {
            0 if bpp >= 2 => (endian_u16(b, o, le) as i16) as f64,
            1 if bpp >= 4 => (endian_u32(b, o, le) as i32) as f64,
            2 => endian_u32(b, o, le) as f64,
            3 => {
                let mut raw = [0u8; 8];
                raw.copy_from_slice(&b[o..o + 8]);
                if !le {
                    raw.reverse();
                }
                u64::from_le_bytes(raw) as f64
            }
            4 if bpp >= 4 => endian_f32(b, o, le) as f64,
            5 if bpp >= 8 => endian_f64(b, o, le),
            6 if bpp >= 1 => b[o] as f64,
            7 if bpp >= 1 => b[o] as i8 as f64,
            _ => return Err("WFM sample format".into()),
        };
        v.push((code * vscale + voff) as f32);
    }
    Ok(v)
}

/// Preamble `:WFMPRE:` and a `:CURVE` block. `Y = (code - YOFF) * YMULT + YZERO`, `t = XZERO + (i - PT_OFF) * XINCR`.
pub fn parse_isf(b: &[u8]) -> Result<Vec<WaveTrace>, String> {
    let s = if b.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    let curve = b[s..].windows(6).position(|w| w == b":CURVE").map(|p| s + p).ok_or("that file holds no waveform")?;
    let mut kv = std::collections::HashMap::<String, String>::new();
    let pre = String::from_utf8_lossy(&b[s..curve]);
    for part in pre.split([';', '\n']) {
        let part = part.trim().trim_start_matches(':');
        let part = part.split_once(':').map(|(_, r)| r.trim()).unwrap_or(part);
        let mut it = part.split_whitespace();
        let Some(key) = it.next() else { continue };
        let val = it.next().unwrap_or("").trim_matches('"').to_string();
        kv.insert(key.to_ascii_uppercase(), val);
    }
    let IsfPreamble { bpp, dt, xzero, pt_off, ymult, yoff, yzero, nr } = IsfPreamble::parse(&kv)?;
    let msb = kv.get("BYT_OR").is_some_and(|s| s.eq_ignore_ascii_case("MSB"));
    let signed = !kv.get("BN_FMT").is_some_and(|s| s.eq_ignore_ascii_case("RP"));
    let ascii = kv.get("ENCDG").is_some_and(|s| s.eq_ignore_ascii_case("ASC"));
    let (raw, vals) = isf_block(b, curve + 6)?;
    let mut codes = isf_codes(&b[raw..raw + vals], bpp, msb, signed, ascii)?;
    let env = kv.get("PT_FMT").is_some_and(|s| s.eq_ignore_ascii_case("ENV"));
    if env && nr == 0 {
        return Err("an envelope needs a waveform point count".into());
    }
    let needed = if env { nr * 2 } else { nr };
    if codes.len() < needed {
        return Err("truncated waveform".into());
    }
    let t0 = xzero - pt_off * dt;
    let scale = |code: f64| ((code - yoff) * ymult + yzero) as f32;
    if env {
        let min: Vec<f32> = codes.iter().take(nr * 2).step_by(2).copied().map(scale).collect();
        let max: Vec<f32> = codes.iter().skip(1).take(nr * 2).step_by(2).copied().map(scale).collect();
        return Ok(vec![
            WaveTrace { t0, dt, v: min, label: "min".into() },
            WaveTrace { t0, dt, v: max, label: "max".into() },
        ]);
    }
    if nr >= 2 && codes.len() > nr {
        codes.truncate(nr);
    }
    if codes.len() < 2 {
        return Err("that file holds no waveform".into());
    }
    Ok(vec![WaveTrace {
        t0,
        dt,
        v: codes.into_iter().map(scale).collect(),
        label: kv.get("WFID").cloned().unwrap_or_default(),
    }])
}

struct IsfPreamble {
    bpp: usize,
    dt: f64,
    xzero: f64,
    pt_off: f64,
    ymult: f64,
    yoff: f64,
    yzero: f64,
    nr: usize,
}

impl IsfPreamble {
    fn parse(kv: &std::collections::HashMap<String, String>) -> Result<Self, String> {
        let num = |k: &str| kv.get(k).and_then(|s| s.parse::<f64>().ok());
        let numeric = |k: &str, default: f64| -> Result<f64, String> {
            match kv.get(k) {
                Some(value) => value.parse().map_err(|_| format!("invalid waveform {k}")),
                None => Ok(default),
            }
        };
        let bpp = numeric("BYT_NR", 1.0)?;
        if !matches!(bpp, 1.0 | 2.0 | 4.0) {
            return Err("that file holds no waveform".into());
        }
        let bpp = bpp as usize;
        let dt = num("XINCR").filter(|x| x.is_finite() && *x > 0.0).ok_or("that file holds no waveform")?;
        let xzero = numeric("XZERO", 0.0)?;
        let pt_off = numeric("PT_OFF", 0.0)?;
        let ymult = numeric("YMULT", 1.0)?;
        let yoff = numeric("YOFF", 0.0)?;
        let yzero = numeric("YZERO", 0.0)?;
        if ![ymult, yoff, yzero, xzero, pt_off].iter().all(|x| x.is_finite()) {
            return Err("that file holds no waveform".into());
        }
        let nr = match kv.get("NR_PT") {
            None => 0,
            Some(value) => value
                .parse::<usize>()
                .ok()
                .filter(|n| (2..=MAX_WAVEFORM_SAMPLES).contains(n))
                .ok_or("invalid waveform point count")?,
        };
        Ok(Self { bpp, dt, xzero, pt_off, ymult, yoff, yzero, nr })
    }
}

fn isf_block(b: &[u8], mut i: usize) -> Result<(usize, usize), String> {
    while b.get(i).is_some_and(u8::is_ascii_whitespace) {
        i += 1;
    }
    if b.get(i) == Some(&b'#') {
        i += 1;
        let n = b
            .get(i)
            .filter(|c| c.is_ascii_digit())
            .map(|c| (c - b'0') as usize)
            .ok_or("invalid waveform block length")?;
        if !(1..=9).contains(&n) || i + 1 + n > b.len() {
            return Err("truncated waveform".into());
        }
        let len: usize =
            std::str::from_utf8(&b[i + 1..i + 1 + n]).ok().and_then(|s| s.parse().ok()).ok_or("truncated waveform")?;
        let start = i + 1 + n;
        if len > MAX_WAVEFORM_BYTES || start.checked_add(len).is_none_or(|end| end > b.len()) {
            return Err(if len > MAX_WAVEFORM_BYTES { "waveform is too large" } else { "truncated waveform" }.into());
        }
        Ok((start, len))
    } else {
        Ok((i, b.len() - i))
    }
}

fn isf_codes(b: &[u8], bpp: usize, msb: bool, signed: bool, ascii: bool) -> Result<Vec<f64>, String> {
    let vals = b.len();
    let mut codes = Vec::new();
    if ascii {
        let text = String::from_utf8_lossy(b);
        for tok in text.split(|c: char| c == ',' || c.is_whitespace()) {
            if let Ok(x) = tok.parse::<f64>() {
                codes.push(x);
            }
        }
    } else {
        if !vals.is_multiple_of(bpp) {
            return Err("truncated waveform".into());
        }
        let n = vals / bpp;
        if n > MAX_WAVEFORM_SAMPLES {
            return Err("waveform is too large".into());
        }
        for k in 0..n {
            let o = k * bpp;
            let code = match bpp {
                1 => {
                    if signed {
                        b[o] as i8 as f64
                    } else {
                        b[o] as f64
                    }
                }
                2 => {
                    let u = endian_u16(b, o, !msb);
                    if signed {
                        u as i16 as f64
                    } else {
                        u as f64
                    }
                }
                _ => {
                    let u = endian_u32(b, o, !msb);
                    if signed {
                        u as i32 as f64
                    } else {
                        u as f64
                    }
                }
            };
            codes.push(code);
        }
    }
    Ok(codes)
}

/// Cookie `AG`, then one header per waveform and a buffer of float32 samples. Little endian.
pub fn parse_ag(b: &[u8]) -> Result<Vec<WaveTrace>, String> {
    if !looks_ag(b) {
        return Err("that file is not a waveform".into());
    }
    let nwave = endian_u32(b, 8, true) as usize;
    let mut p = 12usize;
    let mut out = Vec::with_capacity(nwave);
    for wi in 0..nwave {
        if p + 140 > b.len() {
            return Err("truncated waveform".into());
        }
        let hsize = endian_u32(b, p, true) as usize;
        if !(140..8192).contains(&hsize) || p + hsize > b.len() {
            return Err("truncated waveform".into());
        }
        let nbuf = endian_u32(b, p + 8, true) as usize;
        let npts = endian_u32(b, p + 12, true) as usize;
        let dt = endian_f64(b, p + 32, true);
        let t0 = endian_f64(b, p + 40, true);
        let label = cstr(&b[p + 112..p + 128]);
        if !(1..=32).contains(&nbuf) || !(dt.is_finite() && dt > 0.0 && t0.is_finite()) {
            return Err("that file holds no waveform".into());
        }
        p += hsize;
        let mut normal: Option<Vec<f32>> = None;
        let mut other: Option<Vec<f32>> = None;
        for _ in 0..nbuf {
            if p + 12 > b.len() {
                return Err("truncated waveform".into());
            }
            let dsize = endian_u32(b, p, true) as usize;
            let kind = endian_u16(b, p + 4, true);
            let bpp = endian_u16(b, p + 6, true) as usize;
            let bsize = endian_u32(b, p + 8, true) as usize;
            if dsize < 12 || p + dsize + bsize > b.len() {
                return Err("truncated waveform".into());
            }
            let data = p + dsize;
            if matches!(kind, 1..=3) && bpp == 4 && bsize >= 8 && bsize.is_multiple_of(4) {
                let n = (bsize / 4).min(if npts >= 2 { npts } else { bsize / 4 });
                if (2..=MAX_WAVEFORM_SAMPLES).contains(&n) {
                    let v: Vec<f32> = (0..n).map(|k| endian_f32(b, data + k * 4, true)).collect();
                    if kind == 1 {
                        normal = Some(v);
                    } else if other.is_none() {
                        other = Some(v);
                    }
                }
            }
            p = data + bsize;
        }
        let v = normal.or(other).ok_or("that file holds no waveform")?;
        let label = if label.is_empty() { format!("CH{}", wi + 1) } else { label };
        out.push(WaveTrace { t0, dt, v, label });
    }
    if out.is_empty() {
        return Err("that file holds no waveform".into());
    }
    Ok(out)
}
