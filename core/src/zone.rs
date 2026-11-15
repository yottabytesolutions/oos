// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

/// One zone: a box (2 corner points) or a polygon (3 or more points).
#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub on: bool,
    pub src: usize,
    /// true: the trace must pass through the zone; false: it must not
    pub inside: bool,
    /// (time s, volts)
    pub pts: Vec<(f64, f64)>,
}

impl Default for Zone {
    /// Off, CH1, "must intersect", no outline yet.
    fn default() -> Zone {
        Zone { on: false, src: 0, inside: true, pts: Vec::new() }
    }
}

impl Zone {
    /// The outline as a polygon (a box becomes its 4 corners).
    pub fn polygon(&self) -> Vec<(f64, f64)> {
        match self.pts.len() {
            2 => {
                let ((t0, v0), (t1, v1)) = (self.pts[0], self.pts[1]);
                vec![(t0, v0), (t1, v0), (t1, v1), (t0, v1)]
            }
            _ => self.pts.clone(),
        }
    }

    pub fn usable(&self) -> bool {
        self.on && self.pts.len() >= 2
    }
}
