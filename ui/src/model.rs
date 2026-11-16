// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Compatibility exports and adapter values for the pure domain model.

use crate::board::{board, Active, Board};
use crate::gfx::{rgb, Rgb};
pub(crate) use bridge::config;
pub use bridge::ModelExt;
pub use oos_core::model::*;
pub use protocols::{ProtoExt, PROTOS};

mod bridge;
#[cfg(test)]
mod platform_tests;
mod protocols;

const _: () = assert!(NCH == <Active as Board>::MAX_CHANNELS);
const _: () = assert!(NGEN == <Active as Board>::MAX_GENERATORS);

/// Analog inputs of the unit (`oos_hal::model::platform()`), at most NCH.
pub fn n_ch() -> usize {
    oos_hal::model::platform().caps.channels.min(NCH)
}

/// Built-in generators of the unit (`oos_hal::model::platform()`), at most NGEN.
pub fn gen_count() -> usize {
    oos_hal::model::platform().caps.generators.min(NGEN)
}

/// The memory depths of the board (Acquire menu).
pub fn mdepths() -> &'static [(&'static str, u64)] {
    board().depths()
}

/// Channel names of the unit, "CH1".."CHn" (option lists of channel sources).
pub fn ch_names() -> Vec<String> {
    (0..n_ch()).map(|c| format!("CH{}", c + 1)).collect()
}

/// Optional channel source: `none` first, then the channels.
pub fn opt_ch_names(none: &str) -> Vec<String> {
    std::iter::once(none.to_string()).chain(ch_names()).collect()
}

/// `d` steps from source index `i` among `0..n`, cycling and skipping the channels the unit lacks (2-channel models:
/// CH3/CH4). The steps match the option lists (`ch_names`), so picking an option by its index distance works.
pub fn src_step(i: usize, n: usize, d: i32) -> usize {
    oos_core::config::src_step_in(i, n, d, n_ch())
}

/// The same for an optional channel source encoded as 0 = none, k = channel k - 1.
pub fn opt_src_step(i: usize, d: i32) -> usize {
    oos_core::config::opt_src_step_in(i, d, n_ch())
}

/// Channel colours (yellow, teal, pink, sky blue: a colour-blind safe set). CH2 teal and CH4 blue as the colour
/// rings of the scope's own probes and inputs have them (CH2 cyan, CH4 blue); until 0.1.11 the two were the other way.
const PALETTE: [Rgb; 8] = [
    rgb(0xf0, 0xe4, 0x42),
    rgb(0x2e, 0xc4, 0xa0),
    rgb(0xe0, 0x8c, 0xc0),
    rgb(0x56, 0xb4, 0xe9),
    rgb(0xf0, 0x90, 0x40),
    rgb(0xa0, 0x90, 0xf0),
    rgb(0xc0, 0xd0, 0x60),
    rgb(0xe0, 0x60, 0x60),
];
pub const CH_COLORS: [Rgb; NCH] = {
    let mut c = [PALETTE[0]; NCH];
    let mut i = 0;
    while i < NCH {
        c[i] = PALETTE[i];
        i += 1;
    }
    c
};

pub const MATH_COLOR: Rgb = rgb(0xa9, 0x8b, 0xff);
/// Trace colours of M1..M8 (M1 = the classic math colour).
pub const MATH_COLORS: [Rgb; 8] = [
    MATH_COLOR,
    rgb(0xff, 0x8b, 0xd0),
    rgb(0x8b, 0xe9, 0xff),
    rgb(0xff, 0xc8, 0x6b),
    rgb(0x9b, 0xff, 0x8b),
    rgb(0xff, 0x9b, 0x8b),
    rgb(0xd8, 0xd8, 0xd8),
    rgb(0x8b, 0xa8, 0xff),
];
