// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;
use std::fmt::Debug;

pub(super) const COUPLINGS: [Coupling; 4] = [Coupling::Dc, Coupling::Ac, Coupling::Gnd, Coupling::Dc50];
pub(super) const SLOPES: [Slope; 3] = [Slope::Rise, Slope::Fall, Slope::Either];
pub(super) const TRIG_MODES: [TrigMode; 3] = [TrigMode::Auto, TrigMode::Normal, TrigMode::Single];
pub(super) const ACQ_MODES: [AcqMode; 4] = [AcqMode::Normal, AcqMode::Peak, AcqMode::Average, AcqMode::HiRes];
pub(super) const WHENS: [WhenKind; 5] = [WhenKind::Off, WhenKind::Gt, WhenKind::Lt, WhenKind::In, WhenKind::Out];
pub(super) const WIN_MODES: [WinMode; 3] = [WinMode::Enter, WinMode::Exit, WinMode::Inside];
pub(super) const VSYNCS: [VSync; 5] = [VSync::AllLines, VSync::Line, VSync::Odd, VSync::Even, VSync::AllFields];
pub(super) const HEXPANDS: [HExpand; 4] = [HExpand::Center, HExpand::Left, HExpand::Right, HExpand::Trigger];
pub(super) const CURSORS: [CursorMode; 5] =
    [CursorMode::Off, CursorMode::Time, CursorMode::Volt, CursorMode::Track, CursorMode::Xy];
pub(super) const GRIDS: [Grid; 3] = [Grid::Full, Grid::Light, Grid::None];
pub(super) const VIDEO_STDS: [VideoStd; 2] = [VideoStd::Pal, VideoStd::Ntsc];
pub(super) const BITS: [Bit; 5] = [Bit::H, Bit::L, Bit::X, Bit::R, Bit::F];
pub(super) const POLS: [Pol; 2] = [Pol::Pos, Pol::Neg];
pub(super) const PARITIES: [Parity; 3] = [Parity::None, Parity::Even, Parity::Odd];

pub(super) fn name<T: Debug>(x: T) -> String {
    format!("{:?}", x)
}

pub(super) fn list<T: ToString>(v: &[T]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ")
}

pub(super) fn p_bool(v: &str) -> Result<bool, String> {
    match v {
        "true" | "on" | "1" => Ok(true),
        "false" | "off" | "0" => Ok(false),
        _ => Err(format!("not a boolean: {}", v)),
    }
}

pub(super) fn p_f32(v: &str) -> Result<f32, String> {
    v.parse::<f32>().ok().filter(|x| x.is_finite()).ok_or_else(|| format!("not a number: {}", v))
}

pub(super) fn p_u32(v: &str) -> Result<u32, String> {
    v.parse::<u32>().map_err(|_| format!("not an integer: {}", v))
}

/// Old setup files stored an index into the 5 ns .. 1000 s ladder.
pub(super) fn legacy_tscale(v: &str, ntime: usize) -> Result<usize, String> {
    let five = tscales().iter().position(|&t| t >= 4.9e-9).unwrap_or(0);
    Ok(five.saturating_add(p_idx(v, ntime.saturating_sub(five).max(1))?))
}

/// Index below n.
/// A time/div value as an index into `tscales()` (exact step, 0.1 % tolerance).
pub(super) fn tdiv_index(v: &str) -> Result<usize, String> {
    let x = p_f32(v)?;
    tscales().iter().position(|&s| (s - x).abs() <= s * 1e-3).ok_or_else(|| format!("no time/div step {}", v))
}

pub(super) fn p_idx(v: &str, n: usize) -> Result<usize, String> {
    let i = v.parse::<usize>().map_err(|_| format!("not an index: {}", v))?;
    if i < n {
        Ok(i)
    } else {
        Err(format!("{} out of range (< {})", i, n))
    }
}

pub(super) fn p_decode_source(v: &str) -> Result<usize, String> {
    let source = if v.trim().starts_with(['R', 'r']) {
        match WaveId::parse(v.trim())? {
            WaveId::Ref(i) => DEC_REF_BASE + i,
            _ => unreachable!(),
        }
    } else {
        p_idx(v, DEC_SOURCE_COUNT)?
    };
    decode_wave(source).map(|_| source).ok_or_else(|| "decoder source must be CH or R; LA uses dec_la".into())
}

pub(super) fn p_decode_optional_one_based(v: &str) -> Result<Option<usize>, String> {
    if v.trim() == "none" {
        return Ok(None);
    }
    if v.trim().starts_with(['R', 'r']) {
        return p_decode_source(v).map(Some);
    }
    let source = p_idx(v, DEC_SOURCE_COUNT + 1)?.max(1) - 1;
    decode_wave(source).map(|_| Some(source)).ok_or_else(|| "invalid decoder source".into())
}

pub(super) fn p_enum<T: Debug + Copy>(v: &str, all: &[T]) -> Result<T, String> {
    all.iter().copied().find(|x| format!("{:?}", x) == v).ok_or_else(|| format!("unknown value: {}", v))
}

pub(super) fn p_list<T>(v: &str, n: usize, f: impl Fn(&str) -> Result<T, String>) -> Result<Vec<T>, String> {
    let r: Vec<T> = v.split_whitespace().map(f).collect::<Result<_, _>>()?;
    if r.len() == n {
        Ok(r)
    } else {
        Err(format!("{} values, want {}", r.len(), n))
    }
}
