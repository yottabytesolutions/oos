// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Flat transitional model. Group ownership and change classification arrive in later M2 parts.

mod acquisition;
mod analysers;
mod bus;
mod channel;
mod decoder;
mod defaults;
mod display;
mod generator;
mod settings;
mod signal;
mod state;
mod tables;
mod trigger;
mod wave;

use crate::config::Config;
use crate::math::{MathOp, Window};
use crate::protocols::Context;
pub use crate::protocols::Proto;
pub use acquisition::{AutoAcquisition, AutoDepth};
pub use analysers::*;
pub use bus::*;
pub use channel::*;
pub use decoder::*;
pub use display::*;
pub use generator::*;
pub use state::Model;
pub use tables::*;
pub use trigger::*;
pub use wave::*;

pub const NCH: usize = oos_dsp::CHANNELS;
pub const NGEN: usize = 1;
pub const HDIV: f32 = 10.0;
pub const VDIV: f32 = 8.0;
pub const VIEWS_MAX: usize = 4;
