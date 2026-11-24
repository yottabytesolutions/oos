// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Translate the UI model and configured acquisition into read settings.

use super::{
    roll::slow_mode,
    trigger::{bus_line, hw_trigger, serial_lines_acquired},
};
use crate::{board::Slow, model::*};
use oos_hal::dho::seq::{Chan, Couple, Playback, Segments, Settings, Trig, TriggerOptions};
use oos_hal::dho::{afe, auxout, hz, roll, seq, tpu, uarttrig};

#[cfg(test)]
mod capture_tests;
use super::read_plan;

#[cfg(test)]
pub(super) use read_plan::{live_compress, screen_margin, LIVE_DEPTH};

impl super::super::Dho {
    /// Applied experimental capture geometry or the geometry of a current frame, plus any source diagnostic.
    pub(crate) fn capture_state(&self, source: &crate::wave::Source) -> String {
        let crate::wave::Source::Hw { shared, .. } = source else {
            return " acq.capture=unavailable acq.capture.stored_points=0".into();
        };
        let (applied, mode, points, status, play) = {
            let g = crate::perfguard::deep::ui_lock(shared);
            let id = g.acquisition.identity();
            let frame = &g.frame;
            let current = frame.seq != 0 && (frame.cfg, frame.stamp) == (id.gen(), id.epoch());
            // Device-specific error details stay in the source log.
            let status = g.status.diagnostic(g.acquisition);
            let applied = g.status.capture(id).map(str::to_owned);
            let mode = g.want.as_ref().filter(|_| current).map(|want| want.mode);
            let play =
                g.read_geometry.play.filter(|(played, _)| (played.gen(), played.epoch()) == (id.gen(), id.epoch()));
            (applied, mode, frame.acq.1, status, play)
        };
        let mut text = applied.unwrap_or_else(|| {
            mode.map_or_else(
                || " acq.capture=pending acq.capture.stored_points=0".into(),
                |mode| capture_description(mode, points),
            )
        });
        use std::fmt::Write;
        if let Some(status) = status {
            write!(text, " acq.status={:?}", status.message()).expect("writing to a String cannot fail");
        }
        if let Some((_, play)) = play {
            write!(text, " acq.play.lo={} acq.play.n={} acq.play.k={}", play.lo, play.n, play.k)
                .expect("writing to a String cannot fail");
        }
        text
    }
}

pub(super) fn capture_description(mode: oos_hal::dho::acq::Mode, points: usize) -> String {
    match mode {
        oos_hal::dho::acq::Mode::Reduced(capture) => {
            format!(" acq.capture={capture} acq.capture.stored_points={points}")
        }
        _ => format!(" acq.capture=full acq.capture.stored_points={points}"),
    }
}

/// SINGLE ends with the first frame after the press (`Settings::single`): one frame per batch and no batch after it
/// until the next press. The UI waits for more records for segments, protocol triggers and zone triggers.
fn single_takes_first(m: &Model) -> bool {
    m.trig_mode == TrigMode::Single
        && m.segments == 0
        && m.trig_type != TrigType::Bus
        && !m.zones.iter().any(|z| z.usable())
}
/// The hardware settings for the UI model.
pub fn settings(m: &Model, program_afe: bool, sw: &crate::sysconf::Switches) -> Settings {
    let mask = analog_mask(m);
    let tdiv = m.hw_time_per_div() as f64;
    let slow = slow_mode(m);
    let limit = oos_hal::model::platform().limits.mem(mask.count_ones() as usize, m.la_on());
    let requested = mdepths()[m.mdepth].1 as i128;
    let depth = read_plan::depth(requested, limit as i128, mask, tdiv, m.delay as f64, slow);
    let mut s = Settings {
        // roll and scan have their own way with the position
        delay: if slow == Slow::Off { m.delay as f64 } else { 0.0 },
        mode: if m.acq == AcqMode::Peak { oos_hal::dho::acq::Mode::Peak } else { oos_hal::dho::acq::Mode::Normal },
        holdoff: (m.holdoff as f64).max(oos_hal::dho::seq::HOLDOFF_MIN),
        mask,
        tdiv,
        depth,
        sample_rate: None,
        ch: channels(m),
        afe: program_afe,
        // LA line n: the stock source number 9 + n (trigger.md 2.2)
        trig_src: signal_source(m, mask),
        slope: trigger_slope(m),
        // two-level types: the upper level; all others: the level itself
        level: match m.trig_type {
            TrigType::Slope | TrigType::Runt | TrigType::Window => m.trig_level.max(m.trig_level2),
            TrigType::Bus => m.dec_thr,
            _ => m.trig_level,
        } as f64,
        auto: m.trig_mode == TrigMode::Auto,
        trigger_options: TriggerOptions::new(m.auto_fast && m.trig_mode == TrigMode::Auto, m.trig_nrej),
        single: single_takes_first(m),
        playback: Playback::Raw,
        blocks: 7,
        header: sw.board.header_probe.on(),
        trig_couple: match m.trig_coupling() {
            TrigCoup::Dc => tpu::Coupling::Dc,
            TrigCoup::Ac => tpu::Coupling::Ac,
            TrigCoup::LfReject => tpu::Coupling::LfReject,
            TrigCoup::HfReject => tpu::Coupling::HfReject,
        },
        frames: 1,
        compress: 1,
        trigger: hw_trigger(m, sw.board.serial_hw),
        window: None,
        la: logic_analyser(m, slow),
        segments: (m.seg_hw && m.segments > 0 && hw_segments_ok(m))
            .then_some(Segments { n: m.segments as u32, interval: m.seg_ivl as f64 }),
        aux: aux_out(m.aux_out),
        // the mask test and zone trigger come from the UI (`Shared::mask_want`, `zone_want`), merged in the acquisition
        // thread (`hw_merge`)
        hwmask: None,
        hwzone: None,
    };
    finish_settings(m, sw, slow, &mut s);
    if requested == 0 && slow == Slow::Off && m.auto_acquisition.depth() == AutoDepth::Screen {
        read_plan::use_screen_depth(&mut s);
    }
    s
}

fn logic_analyser(m: &Model, slow: Slow) -> Option<oos_hal::dho::la::Set> {
    use oos_hal::dho::la;
    m.la_on().then(|| la::Set {
        thr: [m.la.thr(0) as f64, m.la.thr(1) as f64],
        analog_shown: m.n_on() > 0,
        cal: la::Cal::default(), // The acquisition thread supplies the calibrated zero codes.
        roll: slow != Slow::Off,
    })
}

/// Planned Auto geometry for hardware readback before a captured frame provides the actual depth.
pub(in crate::board::dho) fn auto_rate_depth(m: &Model, sw: &crate::sysconf::Switches) -> Option<(f64, u64)> {
    if mdepths().get(m.mdepth)?.1 != 0 {
        return None;
    }
    let s = settings(m, false, sw);
    let p = seq::record_geometry(&s).ok()?;
    if slow_mode(m) == Slow::Roll {
        let stream = roll::params(s.mask, seq::tdiv_fs(s.tdiv), s.depth, &p);
        return Some((1e15 / stream.spacing_fs as f64, u64::from(stream.len)));
    }
    Some((p.rate as f64, u64::from(p.memsel)))
}

fn analog_mask(m: &Model) -> u32 {
    let mut mask = (0..NCH).filter(|&c| m.ch[c].on).fold(0u32, |a, c| a | 1 << c);
    // a trigger source is acquired whether it is shown or not (trigger.md 2.1, 2.4)
    mask |= m.trig_channels().iter().enumerate().filter(|(_, &u)| u).fold(0u32, |a, (c, _)| a | 1 << c);
    if mask == 0 {
        // nothing shown: with the LA on the trigger source is acquired hidden (la.md 0.5), else CH1
        mask = if m.la_on() { 1 << m.trig_src.min(NCH - 1) } else { 1 };
    }
    mask
}

fn channels(m: &Model) -> [Chan; NCH] {
    std::array::from_fn(|c| Chan {
        scale: m.ch[c].volts_per_div() as f64,
        offset: m.ch[c].offset as f64,
        bw: bw_code(m.ch[c].bw),
        probe: m.ch[c].probe as f64,
        bias: m.ch[c].bias as f64,
        skew: m.ch[c].skew as f64,
        couple: match m.ch[c].coupling {
            // a board without it never gets it (`oos_core::change::normalize`)
            Coupling::Dc | Coupling::Dc50 => Couple::Dc,
            Coupling::Ac => Couple::Ac,
            Coupling::Gnd => Couple::Gnd,
        },
        invert: m.ch[c].invert,
    })
}

fn signal_source(m: &Model, mask: u32) -> u32 {
    // protocol trigger: hardware edge on the line where frames start (the protocol's start role: SPI chip select, I2S
    // word select, else the data line)
    let mut src = if m.trig_type == TrigType::Bus {
        let Proto::Module(i) = m.dec_proto;
        [Some(m.dec_src), Some(m.dec_src2), m.dec_cs, m.dec_src4][crate::protocols::get(i).start_role().min(3)]
            .unwrap_or(m.dec_src)
    } else {
        m.trig_src
    };
    if mask & (1 << src) == 0 {
        src = mask.trailing_zeros() as usize;
    }
    m.trig_line().or(bus_line(m)).map_or(src as u32 + 1, |n| 9 + n as u32)
}

fn trigger_slope(m: &Model) -> tpu::Slope {
    match (m.trig_type, m.slope) {
        // frames start with a falling edge (start bit, SOF, break, I2C start, CS); I2S and the modules that say so
        // (1553, ARINC 429): either edge
        (TrigType::Bus, _) if m.dec_proto.module().is_some() => match m.dec_proto.module().map(|p| p.start_edge()) {
            Some(Slope::Rise) => tpu::Slope::Rising,
            Some(Slope::Either) => tpu::Slope::Either,
            _ => tpu::Slope::Falling,
        },
        (TrigType::Bus, _) => tpu::Slope::Falling,
        (_, Slope::Rise) => tpu::Slope::Rising,
        (_, Slope::Fall) => tpu::Slope::Falling,
        (_, Slope::Either) => tpu::Slope::Either,
    }
}

fn finish_settings(m: &Model, sw: &crate::sysconf::Switches, slow: Slow, s: &mut Settings) {
    // the hardware UART fires inside the frame, the software trigger point is the end of its stop bit: the record is
    // centred on the software point (docs/fork/hwuart.md)
    if let (Trig::Uart(u), Slow::Off) = (&s.trigger, slow) {
        s.delay += uarttrig::sw_offset_s(u);
    }
    // the SPI and I2C receivers need every line they use among the acquired channels; else the edge trigger
    if !serial_lines_acquired(&s.trigger, s.mask) {
        s.trigger = Trig::Edge;
    }
    read_plan::configure_capture(m, sw, slow, s);
}

/// Protocol watch and XY need actual stored samples.
#[cfg(any(target_os = "linux", test))]
pub(super) fn raw_for_ui(s: &mut Settings, needs_raw: bool) {
    if needs_raw {
        s.compress = 1;
    }
}

/// The batch was read with the live compression (`live_compress`): its newest frame is read again raw
/// (`seq::time::read_raw_again_with_clock`) for the record the UI keeps.
#[cfg(any(target_os = "linux", test))]
pub(in crate::board::dho) fn raw_newest(b: &Settings, p: &hz::Params) -> bool {
    read_plan::ReadPlan::from_batch(b, p).raw_newest()
}

/// The AUX OUT set-up of the menu choice. Pass/fail takes the default pulse of `auxout::PassFail` (docs/fork/auxout.md).
fn aux_out(a: AuxOut) -> auxout::AuxOut {
    match a {
        AuxOut::TrigOut => auxout::AuxOut::TriggerOut,
        AuxOut::Off => auxout::AuxOut::Off,
        AuxOut::PassFail => auxout::AuxOut::PassFail(auxout::PassFail::DEFAULT),
    }
}

/// The model's set-up averages in the FPGA (`Board::avg_hw`). The switches do not change the mode.
pub fn avg_hw_ok(m: &Model) -> bool {
    matches!(settings(m, false, &crate::sysconf::Switches::default()).mode, oos_hal::dho::acq::Mode::Average { .. })
}

/// Hardware segments record ordinary triggered frames: not in roll or scan, not with the LA (its record layout and
/// read are not worked out for them).
fn hw_segments_ok(m: &Model) -> bool {
    slow_mode(m) == Slow::Off && !m.la_on()
}

/// Most hardware segments for the model's set-up (`Board::seg_max`): the whole-memory limit of its segment size. The
/// switches do not change the depth.
pub fn seg_max(m: &Model) -> Option<usize> {
    if !hw_segments_ok(m) {
        return None;
    }
    let mut s = settings(m, false, &crate::sysconf::Switches::default());
    s.mode = oos_hal::dho::acq::Mode::Normal;
    let p = seq::record_geometry(&s).ok()?;
    Some(hz::max_record_frames(p.seg) as usize)
}

/// Settings for reading hardware segments back: the live batch plan (compression above 1 M points) with the header
/// on, at most ~4 MB and half the UI queue per read.
#[cfg(any(target_os = "linux", test))]
pub fn seg_read_plan(s: &Settings, p: &hz::Params) -> Settings {
    // the frames per read come from `read_settings`
    let b = batch_plan(s, p, 1);
    let fb = oos_hal::dho::seq::frame_bytes(&oos_hal::dho::seg::read_settings(&b, 1), p).max(1);
    let k = ((4_000_000 / fb) as u32).clamp(1, (crate::board::QUEUE_MAX / 2) as u32);
    oos_hal::dho::seg::read_settings(&b, k)
}

/// hal bandwidth code of a BWS entry.
pub fn bw_code(i: usize) -> u32 {
    match i {
        0 => afe::bw::AUTO,
        BW_FULL => afe::bw::OFF,
        BW_20M => afe::bw::S20M,
        3 => afe::bw::GND,
        _ => afe::bw::mhz(BWS[i.min(BWS.len() - 1)].1),
    }
}

/// Batch settings for a configured acquisition.
#[cfg(any(target_os = "linux", test))]
pub fn batch_plan(s: &Settings, p: &hz::Params, zone_batch: u32) -> Settings {
    super::window::batch_play(s, p, zone_batch, None)
}
