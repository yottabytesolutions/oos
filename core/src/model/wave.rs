// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Source identities: channels and stored reference records (R1..R10), and the decoder and measurement selections
//! that name them.

use super::NCH;

/// Reference waveform slots, R1..R10. A slot is waveform memory, not a channel index.
pub const NREF: usize = 10;
/// Decoder namespace: CH0..3 and reserved legacy LA4..19 stay unchanged.
/// LA decoding still uses dec_la_in/dec_la; stored roles append R1..R10.
pub const DEC_REF_BASE: usize = NCH + 16;
pub const DEC_SOURCE_COUNT: usize = DEC_REF_BASE + NREF;

pub fn decode_wave(source: usize) -> Option<WaveId> {
    if source < NCH {
        Some(WaveId::Ch(source))
    } else if (DEC_REF_BASE..DEC_SOURCE_COUNT).contains(&source) {
        Some(WaveId::Ref(source - DEC_REF_BASE))
    } else {
        None
    }
}

pub fn decode_source_name(source: usize) -> String {
    match decode_wave(source) {
        Some(WaveId::Ch(c)) => format!("CH{}", c + 1),
        Some(WaveId::Ref(i)) => format!("R{}", i + 1),
        None => "Unavailable".into(),
    }
}

/// Total selected single, pair and catalog rows, shared by UI and setup validation.
pub const MEAS_SELECTION_MAX: usize = 1024;

/// Persisted catalog IDs are only the extra Wave rows; ordinary Meas/pairs retain their existing formats.
pub fn catalog_id_valid(id: u16) -> bool {
    matches!(id, 15..=16 | 40..=51 | 60..=66 | 70..=76 | 80..=82 | 90..=93
        | 100..=118 | 130..=137 | 150..=153 | 170..=186)
}

/// A waveform a tool can name. A channel is the live front end. A ref is one stored record, measured at
/// that record's sample interval. Refs are not packed into the channel/math index.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum WaveId {
    Ch(usize),
    Ref(usize),
}

impl WaveId {
    pub fn ch(self) -> Option<usize> {
        match self {
            WaveId::Ch(c) => Some(c),
            WaveId::Ref(_) => None,
        }
    }

    /// Setup text: `1`..`4` are channels, `R1`..`R10` are refs. Old files only have channel numbers.
    pub fn token(self) -> String {
        match self {
            WaveId::Ch(c) => (c + 1).to_string(),
            WaveId::Ref(i) => format!("R{}", i + 1),
        }
    }

    /// Inverse of `token`. `0` is CH1, the same clamp the old channel parser used.
    pub fn parse(v: &str) -> Result<WaveId, String> {
        if let Some(rest) = v.strip_prefix('R').or_else(|| v.strip_prefix('r')) {
            let n = rest.parse::<usize>().map_err(|_| format!("not a source: {}", v))?;
            if (1..=NREF).contains(&n) {
                Ok(WaveId::Ref(n - 1))
            } else {
                Err(format!("R{} out of range", n))
            }
        } else {
            let i = v.parse::<usize>().map_err(|_| format!("not a source: {}", v))?;
            if i == 0 {
                Ok(WaveId::Ch(0))
            } else if i <= NCH {
                Ok(WaveId::Ch(i - 1))
            } else {
                Err(format!("{} out of range", i))
            }
        }
    }
}

/// 1-2-5 steps from `lo` to `hi`. `zero` inserts 0 (an unset geometry or resistance). `d` moves along that list and
/// stops at the ends.
pub fn eng_ladder(x: f64, d: i32, lo: f64, hi: f64, zero: bool) -> f64 {
    let mut l = Vec::new();
    if zero {
        l.push(0.0);
    }
    let mut e = lo;
    while e <= hi * 1.001 && l.len() < 48 {
        l.push(e);
        l.push(e * 2.0);
        l.push(e * 5.0);
        e *= 10.0;
    }
    if l.is_empty() {
        return x;
    }
    let i = l.iter().position(|&v| v + 1e-12 >= x).unwrap_or(l.len() - 1);
    l[(i as i32 + d).clamp(0, l.len() as i32 - 1) as usize]
}
