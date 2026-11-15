// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::VSCALES;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Coupling {
    Dc,
    Ac,
    Gnd,
    /// DC into 50 ohms, on boards that offer it (`Board::couplings`)
    Dc50,
}

impl Coupling {
    pub fn name(self) -> &'static str {
        match self {
            Coupling::Dc => "DC",
            Coupling::Ac => "AC",
            Coupling::Gnd => "GND",
            Coupling::Dc50 => "DC50",
        }
    }
}

/// Coupling of the edge trigger: a filter in the trigger path only, the trace stays as the channel's coupling shows it
/// (trigger.md 2.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrigCoup {
    Dc,
    Ac,
    LfReject,
    HfReject,
}

pub const TRIG_COUPS: [TrigCoup; 4] = [TrigCoup::Dc, TrigCoup::Ac, TrigCoup::LfReject, TrigCoup::HfReject];

/// What the rear AUX OUT BNC carries (System > AUX out, `:SYSTem:AOUTput`, docs/fork/auxout.md).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AuxOut {
    /// a pulse per trigger: the FPGA's power-up state, so the default
    TrigOut,
    /// constant low
    Off,
    /// a pulse per failed acquisition of the hardware mask test (`mask.hw = on`, docs/fork/hwmask.md); without it the
    /// output stays low
    PassFail,
}

pub const AUX_OUTS: [AuxOut; 3] = [AuxOut::TrigOut, AuxOut::Off, AuxOut::PassFail];

impl AuxOut {
    /// what the menu and SCPI take: pass/fail only where the mask test or the zone trigger can run in hardware
    /// (`Board::mask_hw`, `Board::zone_hw`); a setup file takes every value
    pub fn offered(self, hw_mask: bool) -> bool {
        self != AuxOut::PassFail || hw_mask
    }

    /// the offered choices in menu order
    pub fn choices(hw_mask: bool) -> Vec<AuxOut> {
        AUX_OUTS.iter().copied().filter(|a| a.offered(hw_mask)).collect()
    }

    pub fn name(self) -> &'static str {
        match self {
            AuxOut::TrigOut => "Trigger out",
            AuxOut::Off => "Off",
            AuxOut::PassFail => "Pass/fail",
        }
    }
}

impl TrigCoup {
    pub fn name(self) -> &'static str {
        match self {
            TrigCoup::Dc => "DC",
            TrigCoup::Ac => "AC",
            TrigCoup::LfReject => "LF reject",
            TrigCoup::HfReject => "HF reject",
        }
    }
}

/// Deskew range (s).
pub const SKEW_MAX: f32 = 100e-9;

/// Unit of a channel.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AmpUnit {
    V,
    A,
    W,
    U,
}

impl AmpUnit {
    pub const ALL: [AmpUnit; 4] = [AmpUnit::V, AmpUnit::A, AmpUnit::W, AmpUnit::U];
    pub fn name(self) -> &'static str {
        ["V", "A", "W", "U"][self as usize]
    }
    /// A measurement's unit on such a channel: the V in it replaced.
    pub fn of(self, unit: &'static str) -> &'static str {
        match (self, unit) {
            (AmpUnit::A, "V") => "A",
            (AmpUnit::A, "Vs") => "As",
            (AmpUnit::A, "V/s") => "A/s",
            (AmpUnit::W, "V") => "W",
            (AmpUnit::W, "Vs") => "Ws",
            (AmpUnit::W, "V/s") => "W/s",
            (AmpUnit::U, "V") => "U",
            (AmpUnit::U, "Vs") => "Us",
            (AmpUnit::U, "V/s") => "U/s",
            _ => unit,
        }
    }
}

/// What stays put on the screen when the time base changes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum HExpand {
    Center,
    Left,
    Right,
    Trigger,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Channel {
    pub on: bool,
    pub vscale: usize, // index into VSCALES
    /// fine scale: factor on the step of `vscale`, 1.0 = exactly on the step
    pub vfine: f32,
    /// what the channel measures: only the unit it shows
    pub unit: AmpUnit,
    /// zero correction in V: subtracted from what the channel reads
    pub bias: f32,
    /// deskew in s
    pub skew: f32,
    pub offset: f32, // volts added before display (vertical position)
    pub coupling: Coupling,
    /// bandwidth limit, index into BWS
    pub bw: usize,
    pub probe: f32,
    pub invert: bool,
}

impl Channel {
    pub fn volts_per_div(&self) -> f32 {
        VSCALES[self.vscale] * self.vfine * self.probe
    }

    /// Largest offset of the scale at the probe tip: the input gain path has its own range (afe-settings.md 3.2), the
    /// file's limits vary a little from block to block.
    pub fn offset_max(&self) -> f32 {
        let bnc = VSCALES[self.vscale] * self.vfine;
        self.probe
            * if bnc < 0.06 {
                2.0
            } else if bnc < 0.26 {
                12.0
            } else if bnc < 2.65 {
                56.0
            } else {
                550.0
            }
    }
}
