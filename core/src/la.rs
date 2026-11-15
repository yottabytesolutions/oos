// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Logic analyser settings and record values.

use std::sync::Arc;

/// Threshold presets: (name, volts). The last entry is the user value (`LaSet::user`).
pub const PRESETS: [(&str, f32); 8] = [
    ("TTL 1.4 V", 1.4),
    ("CMOS 5 V: 2.5 V", 2.5),
    ("CMOS 3.3 V: 1.65 V", 1.65),
    ("LVCMOS 2.5 V: 1.25 V", 1.25),
    ("LVCMOS 1.8 V: 0.9 V", 0.9),
    ("LVCMOS 1.2 V: 0.6 V", 0.6),
    ("ECL -1.3 V", -1.3),
    ("User", f32::NAN),
];
/// Index of the user threshold in PRESETS.
pub const USER: usize = PRESETS.len() - 1;
pub const HEIGHTS: [(&str, i32); 3] = [("Small", 12), ("Medium", 16), ("Large", 22)];
pub const THR_MAX: f32 = 15.0;

/// LA settings of the UI model (part of setups).
#[derive(Clone, Debug, PartialEq)]
pub struct LaSet {
    pub on: bool,
    /// lines shown: bit n = Dn (the hardware records all 16 anyway, la.md 0.1)
    pub show: u16,
    /// threshold preset per pod (index into PRESETS)
    pub preset: [usize; 2],
    /// user threshold per pod in volts
    pub user: [f32; 2],
    /// index into HEIGHTS
    pub height: usize,
}

impl Default for LaSet {
    fn default() -> LaSet {
        LaSet { on: false, show: 0xffff, preset: [0, 0], user: [1.65, 1.65], height: 1 }
    }
}

impl LaSet {
    /// Threshold of pod `p` (0 = D0-D7, 1 = D8-D15) in volts.
    pub fn thr(&self, p: usize) -> f32 {
        let k = self.preset[p].min(USER);
        if k == USER {
            self.user[p]
        } else {
            PRESETS[k].1
        }
    }

    /// The lines shown, D0 first.
    pub fn lines(&self) -> Vec<usize> {
        (0..16).filter(|n| self.show >> n & 1 == 1).collect()
    }

    /// Setup value: `on show preset1 preset2 user1 user2 height` (show in hex).
    pub fn to_text(&self) -> String {
        format!(
            "{} {:04x} {} {} {} {} {}",
            self.on, self.show, self.preset[0], self.preset[1], self.user[0], self.user[1], self.height
        )
    }

    pub fn parse(v: &str) -> Result<LaSet, String> {
        let w: Vec<&str> = v.split_whitespace().collect();
        if w.len() != 7 {
            return Err("la: want 7 values".into());
        }
        let b = |s: &str| match s {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(format!("not a boolean: {}", s)),
        };
        let idx =
            |s: &str, n: usize| s.parse::<usize>().ok().filter(|&i| i < n).ok_or_else(|| format!("bad index {}", s));
        let volt = |s: &str| {
            s.parse::<f32>()
                .ok()
                .filter(|x| x.is_finite())
                .map(|x| x.clamp(-THR_MAX, THR_MAX))
                .ok_or_else(|| format!("not a number: {}", s))
        };
        Ok(LaSet {
            on: b(w[0])?,
            show: u16::from_str_radix(w[1], 16).map_err(|e| e.to_string())?,
            preset: [idx(w[2], PRESETS.len())?, idx(w[3], PRESETS.len())?],
            user: [volt(w[4])?, volt(w[5])?],
            height: idx(w[6], HEIGHTS.len())?,
        })
    }
}

/// The 16 lines of a record as packed words: word k (bit n = Dn) at time t0 + k dt relative to the trigger.
#[derive(Clone, Debug)]
pub struct LaRec {
    pub words: Arc<Vec<u16>>,
    pub t0: f64,
    pub dt: f64,
}
