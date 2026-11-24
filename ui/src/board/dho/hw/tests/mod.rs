// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    comparator::{hw_merge, is_stall, logs_acquire_error, unit_key, UnitGuard, UNIT_RETRY},
    plan::{live_compress, raw_for_ui, raw_newest, screen_margin, LIVE_DEPTH},
    roll::{scan_gap, scan_la_pos, scan_write, SCAN_TDIV},
    selfcal::selfcal_saved,
    trigger::{i2c_of, serial_lines_acquired, spi_of, uart_debug, uart_of},
    trim::{TrimPending, TRIM_HOLD},
    units::{dvm_step, DvmStep},
    window::{deep_raw_settings, prearm_keeps_replay},
    *,
};
use crate::{
    board::{to_record, Frame, Slow},
    model::*,
};
use oos_hal::dho::{
    auxout, hz,
    seq::{Chan, Segments, Settings, Trig},
    uarttrig,
};
use std::sync::Arc;

mod comparator;
mod compression;
mod decode;
mod deep;
mod geometry;
mod mask;
mod plan;
mod replay;
mod roll;
mod service;
mod single;
mod trigger;
mod units;
mod windows;

fn seq_bytes(b: &Settings, p: &hz::Params) -> usize {
    oos_hal::dho::seq::frame_bytes(b, p)
}
