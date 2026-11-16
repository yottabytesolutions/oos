// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::Proto;

/// Settings of one decoder (the `dec_*` fields of `Model` are the one the menu edits).
#[derive(Clone, PartialEq, Debug)]
pub struct DecCfg {
    pub on: bool,
    pub src: usize,
    pub baud: usize,
    pub canrate: usize,
    pub canh: bool,
    pub fdrate: usize,
    pub arate: usize,
    pub tick: usize,
    pub nibbles: u32,
    pub uspeed: usize,
    pub cphy_rate: f64,
    pub src4: Option<usize>,
    pub la_in: bool,
    pub la: [Option<u8>; 4],
    pub diff: Option<usize>,
    pub thr: f32,
    pub ascii: bool,
    pub proto: Proto,
    pub src2: usize,
    pub cs: Option<usize>,
    pub mode: u8,
    pub bits: u32,
    pub parity: oos_dsp::decode::Parity,
    pub opt: [u8; crate::protocols::OPTS],
}

/// Decoders at once.
pub const DECODERS: usize = 4;
