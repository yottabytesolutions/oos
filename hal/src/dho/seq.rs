// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Complete acquisition sequences: settings -> ordered steps (register writes, AFE frames, polls, sleeps, the C2H
//! read), a persistent state with all shadows, the executor for the unit, and read-out layout. Shared by `oos-fpgad`
//! and the hardware source of `oos-ui`. Order (docs/protocols): [ADC mode] -> [AFE] -> TPU edge trigger -> wait FSM
//! stopped -> acquisition configuration -> SCU auto/normal trigger -> arm -> wait done -> play-last read-out -> read.

mod arm;
mod capture;
mod execute;
mod readout;
mod settings;
mod state;
pub mod time;
mod wait;

pub use super::samples::{
    code_map, decode_volts, decode_volts_into, deinterleave, lane_volts_into, volt_code, volts_f64 as volts, CodeMap,
};
pub use arm::{frames, record_frames, CaptureDisarm, CaptureStop};
pub use capture::CaptureReduction;
#[cfg(target_os = "linux")]
pub use execute::linux::{execute, execute_into_until, execute_until};
pub use execute::{
    acquire_batch, acquire_batch_idle, acquire_batch_until, acquire_batch_with_clock, execute_bus, execute_bus_until,
};
pub use readout::{
    compress_k, compress_word, compressed_len, frame_bytes, frame_stride, read_len, unpad, unpad_in, window, Playback,
    COMPRESS_K_MAX, STAMP_BYTES, STAMP_WORDS,
};
pub use settings::{
    raw_settings, tdiv_fs, time_fs, Chan, Couple, HwMask, HwZone, Segments, Settings, Trig, HOLDOFF_MIN,
};
pub use state::{plan, State};
pub use wait::{wait_steps, NEW_SETTINGS};

use super::{afe, Write};

#[cfg(test)]
mod test_support;

#[derive(Clone, Debug)]
pub enum Step {
    Wr(Write),
    Afe(afe::Op),
    /// wait until (reg & bit) != 0, at most `ms`
    Poll {
        reg: u32,
        bit: u32,
        ms: u64,
        what: &'static str,
    },
    /// read-only diagnostic: log the values of these registers
    Peek(&'static [u32]),
    /// `frames` frames of `frame` bytes each; in the stream every frame is padded to `frame_stride(frame)` (8 bytes),
    /// the read takes the padding too and drops it
    Read {
        frames: usize,
        frame: usize,
    },
    Sleep {
        us: u32,
    },
    Note(String),
}
