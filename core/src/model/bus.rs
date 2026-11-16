// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The bus drawn on the plot.

use super::WaveId;

/// How far a bus may sit from the middle of the grid, in divisions. Positive is up.
pub const BUS_POS_MIN: f32 = -3.6;
pub const BUS_POS_MAX: f32 = 3.6;

/// The bus drawn on the plot. Only I2C is defined.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BusKind {
    I2c,
}

impl BusKind {
    pub fn name(self) -> &'static str {
        match self {
            BusKind::I2c => "I2C",
        }
    }
}

/// One bus on the waveform: on or off, a label, and a vertical position in divisions.
#[derive(PartialEq, Debug)]
pub struct TraceBus {
    pub on: bool,
    pub label: String,
    /// divisions from the center of the grid, positive up
    pub pos: f32,
    pub kind: BusKind,
    /// clock (SCL) and data (SDA)
    pub clk: WaveId,
    pub data: WaveId,
    pub clk_thr: f32,
    pub data_thr: f32,
    /// the address word includes the read/write bit as its least significant bit
    pub addr_rw: bool,
}

impl Default for TraceBus {
    fn default() -> Self {
        Self {
            on: false,
            label: "I2C".into(),
            pos: -3.0,
            kind: BusKind::I2c,
            clk: WaveId::Ch(0),
            data: WaveId::Ch(1),
            clk_thr: 1.65,
            data_thr: 1.65,
            addr_rw: false,
        }
    }
}

/// `clone_from` keeps the label's buffer: change snapshots refresh this field every step without allocating.
impl Clone for TraceBus {
    fn clone(&self) -> Self {
        Self { label: self.label.clone(), ..*self }
    }

    fn clone_from(&mut self, source: &Self) {
        let TraceBus { on, label, pos, kind, clk, data, clk_thr, data_thr, addr_rw } = source;
        self.label.clone_from(label);
        (self.on, self.pos, self.kind, self.clk, self.data) = (*on, *pos, *kind, *clk, *data);
        (self.clk_thr, self.data_thr, self.addr_rw) = (*clk_thr, *data_thr, *addr_rw);
    }
}

impl TraceBus {
    /// The name drawn on the plot. An empty label falls back to the type.
    pub fn shown_label(&self) -> String {
        let t = self.label.trim();
        if t.is_empty() {
            self.kind.name().to_string()
        } else {
            t.to_string()
        }
    }
}

/// Keeps a bus label to the length of a channel name, without control characters.
pub fn bus_label(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).take(10).collect::<String>().trim().to_string()
}
