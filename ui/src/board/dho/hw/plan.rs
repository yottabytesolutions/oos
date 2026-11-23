// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Translate the UI model and configured acquisition into read settings.

use super::{
    roll::slow_mode,
    trigger::{bus_line, hw_trigger, serial_lines_acquired},
};
use crate::{board::Slow, model::*};
use oos_hal::dho::seq::{Chan, Couple, Playback, Segments, Settings, Trig};
use oos_hal::dho::{afe, auxout, hz, tpu, uarttrig};

#[cfg(test)]
mod capture_tests;

impl super::super::Dho {
    /// Capture mode and stored depth confirmed by a frame of the current hardware settings.
    pub(crate) fn capture_state(&self, source: &crate::wave::Source) -> String {
        use oos_hal::dho::acq::Mode;
        let crate::wave::Source::Hw { shared, .. } = source else {
            return " acq.capture=unavailable acq.capture.stored_points=0".into();
        };
        let (mode, points) = {
            let g = crate::perfguard::deep::ui_lock(shared);
            let id = g.acquisition.identity();
            let frame = &g.frame;
            let current = frame.seq != 0 && (frame.cfg, frame.stamp) == (id.gen(), id.epoch());
            (g.want.as_ref().filter(|_| current).map(|want| want.mode), frame.acq.1)
        };
        match mode {
            Some(Mode::Reduced(capture)) => format!(" acq.capture={capture} acq.capture.stored_points={points}"),
            Some(_) => format!(" acq.capture=full acq.capture.stored_points={points}"),
            None => " acq.capture=pending acq.capture.stored_points=0".into(),
        }
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
/// Live record length for Auto depth (keeps the read short enough for a fluid display).
pub(super) const LIVE_DEPTH: i128 = 100_000;

/// Auto depth at short time bases: the stock record is exactly one screen at the full rate, so after the software
/// trigger moves the record by a few samples the edge of the screen has no data (acquisition-start.md, C2H read rules
/// 3). An eighth more memory (at least 64 points, a multiple of 8) at the same rate: `hz::prep` keeps the full rate for
/// a depth above one screen and centres the record on the trigger.
pub(super) fn screen_margin(memsel: i128) -> i128 {
    (memsel + (memsel / 8).max(64) + 7) / 8 * 8
}

/// Auto depth with a trigger delay: the record is centred on the delayed screen, but the trigger has to stay inside it
/// (`seq::State::configure` clamps the delay to half a record, trigger.md 4). A delay beyond half of the `base` record
/// then showed only the part of the screen up to the record's end (10 us/div, 200 us delay: 20 us of 100, unit
/// 2026-11-13). The record then gets the depth that reaches from the trigger (a division inside) past the delay at the
/// full rate; the depth limit of the channels still applies afterwards.
fn delay_depth(mask: u32, tdiv: f64, delay: f64, base: i128) -> i128 {
    let p = hz::prep(mask, oos_hal::dho::seq::tdiv_fs(tdiv), base, 0);
    let span = p.memsel as f64 / p.rate.max(1) as f64;
    if !delay.is_finite() || 2.0 * delay.abs() <= span {
        return base;
    }
    let full = (hz::CLOCK / hz::ratio(mask) as i128) as f64;
    // at most the deepest record of the board; `settings` then limits it to the channels' memory
    let want = ((2.0 * delay.abs() + 2.0 * tdiv) * full).ceil().min(125e6) as i128;
    ((want + 7) / 8 * 8).max(base)
}

/// The hardware settings for the UI model.
pub fn settings(m: &Model, program_afe: bool, sw: &crate::sysconf::Switches) -> Settings {
    let mask = analog_mask(m);
    let tdiv = m.hw_time_per_div() as f64;
    let slow = slow_mode(m);
    let depth = depth(m, mask, tdiv, slow);
    let la = m.la_on().then(|| oos_hal::dho::la::Set {
        thr: [m.la.thr(0) as f64, m.la.thr(1) as f64],
        analog_shown: m.n_on() > 0,
        cal: oos_hal::dho::la::Cal::default(), // The acquisition thread supplies the calibrated zero codes.
        roll: slow != Slow::Off,
    });
    let mut s = Settings {
        // roll and scan have their own way with the position
        delay: if slow == Slow::Off { m.delay as f64 } else { 0.0 },
        mode: if m.acq == AcqMode::Peak { oos_hal::dho::acq::Mode::Peak } else { oos_hal::dho::acq::Mode::Normal },
        holdoff: (m.holdoff as f64).max(oos_hal::dho::seq::HOLDOFF_MIN),
        mask,
        tdiv,
        depth,
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
        auto_fast: m.auto_fast && m.trig_mode == TrigMode::Auto,
        single: single_takes_first(m),
        playback: Playback::Raw,
        blocks: 7,
        header: sw.board.header_probe.on(),
        noise_reject: m.trig_nrej,
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
        la,
        segments: (m.seg_hw && m.segments > 0 && hw_segments_ok(m))
            .then_some(Segments { n: m.segments as u32, interval: m.seg_ivl as f64 }),
        aux: aux_out(m.aux_out),
        // the mask test and zone trigger come from the UI (`Shared::mask_want`, `zone_want`), merged in the acquisition
        // thread (`hw_merge`)
        hwmask: None,
        hwzone: None,
    };
    finish_settings(m, sw, slow, &mut s);
    s
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

fn depth(m: &Model, mask: u32, tdiv: f64, slow: Slow) -> i128 {
    let depth = match mdepths()[m.mdepth].1 {
        // roll streams without reading the memory: the stock Auto depth gives the stock point spacing (10 us at
        // 100 ms/div)
        0 if slow != Slow::Off => 0,
        0 => {
            let auto = hz::prep(mask, oos_hal::dho::seq::tdiv_fs(tdiv), 0, 0);
            let base = if auto.memsel as i128 > LIVE_DEPTH { LIVE_DEPTH } else { screen_margin(auto.memsel as i128) };
            delay_depth(mask, tdiv, m.delay as f64, base)
        }
        // roll and scan: a fixed depth below 1 M becomes 1 M (roll-mode.md 1.3)
        d if slow != Slow::Off => (d as i128).max(1_000_000),
        d => d as i128,
    };
    // the depth of the menu may be from fewer channels: never beyond the limit of the channels on now
    let max_mem = oos_hal::model::platform().limits.mem(mask.count_ones() as usize, m.la_on());
    let depth = if depth > 0 { depth.min(max_mem as i128) } else { depth };
    // roll faster than 50 ms/div: no more than the roll stream's rate
    let df = oos_hal::dho::roll::depth_for(oos_hal::dho::seq::tdiv_fs(tdiv));
    if slow == Slow::Roll && df > 0 {
        if depth == 0 {
            df
        } else {
            depth.min(df)
        }
    } else {
        depth
    }
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
    if let Some(count) = hw_avg_count(m, s) {
        s.mode = oos_hal::dho::acq::Mode::Average { count };
    }
    capture_reduction(m, sw, slow, s);
    if sw.compress {
        live_compress(m, s);
    }
}

fn capture_reduction(m: &Model, sw: &crate::sysconf::Switches, slow: Slow, s: &mut Settings) {
    let Some((peak, factor, inferred)) = sw.source.capture().reduction() else { return };
    if m.acq == AcqMode::Normal
        && slow == Slow::Off
        && m.mdepth == 0
        && m.trig_type == TrigType::Edge
        && m.segments == 0
        && !m.mask_on
        && !m.zones.iter().any(|z| z.usable())
    {
        let _ = s.reduce_capture(peak, factor, inferred);
    }
}

/// The live compression for the model (compress.rs): the factor of `compress::k_for` for the record of `s` where the
/// model allows it (not in roll or scan), else none.
pub(super) fn live_compress(m: &Model, s: &mut Settings) {
    if slow_mode(m) != Slow::Off
        || !crate::compress::allowed(m)
        || s.la.is_some()
        || s.segments.is_some()
        || !matches!(s.mode, oos_hal::dho::acq::Mode::Normal)
    {
        return;
    }
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
    s.compress = crate::compress::k_for(p.rate, s.tdiv, p.memsel);
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
pub(in crate::board::dho) fn raw_newest(b: &Settings, p: &hz::Params) -> bool {
    b.compress > 1 && p.memsel <= crate::compress::MAX_DEPTH
}

/// The AUX OUT set-up of the menu choice. Pass/fail takes the default pulse of `auxout::PassFail` (docs/fork/auxout.md).
fn aux_out(a: AuxOut) -> auxout::AuxOut {
    match a {
        AuxOut::TrigOut => auxout::AuxOut::TriggerOut,
        AuxOut::Off => auxout::AuxOut::Off,
        AuxOut::PassFail => auxout::AuxOut::PassFail(auxout::PassFail::DEFAULT),
    }
}

/// The count of a hardware average for the model and its settings `s`, None for the software average
///: Average mode with `Model::avg_hw`, a power-of-two count, ordinary triggered
/// records (not roll or scan, which average in software as in stock), no LA, no hardware segments (they record raw
/// frames), and a record whose average segment fits the frame ring.
fn hw_avg_count(m: &Model, s: &Settings) -> Option<u32> {
    if m.acq != AcqMode::Average || !m.avg_hw || slow_mode(m) != Slow::Off || s.la.is_some() || s.segments.is_some() {
        return None;
    }
    let p = hz::avg_params(hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0));
    oos_hal::dho::avg::hw_count(m.avg_count).filter(|_| p.max_frames > 0)
}

/// The model's set-up averages in the FPGA (`Board::avg_hw`). The switches do not change the mode.
pub fn avg_hw_ok(m: &Model) -> bool {
    matches!(settings(m, false, &crate::sysconf::Switches::default()).mode, oos_hal::dho::acq::Mode::Average { .. })
}

/// The `cal.core_offset` line; `tight` or not.
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
    let s = settings(m, false, &crate::sysconf::Switches::default());
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
    Some(hz::max_record_frames(p.seg) as usize)
}

/// Settings for reading hardware segments back: the live batch plan (compression above 1 M points) with the header
/// on, at most ~4 MB and half the UI queue per read.
pub fn seg_read_plan(s: &Settings, p: &hz::Params) -> Settings {
    // the frames per read come from `read_settings`
    let b = batch_plan(s, p, 1);
    let fb = oos_hal::dho::seq::frame_bytes(&oos_hal::dho::seg::read_settings(&b, 1), p).max(1);
    let k = ((4_000_000 / fb) as u32).clamp(1, (crate::board::QUEUE_MAX / 2) as u32);
    oos_hal::dho::seg::read_settings(&b, k)
}

/// Time of the first LA word of a roll frame relative to frame point `pre`, so that the newest word (index `n_la` - 1,
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

/// Points of a live frame read from a record above 1 M points (the stock live length at 2.5 M / k = 4).
pub(super) const LIVE_READ: u32 = 625_000;

/// Batch settings for a configured acquisition: frames per batch (at most ~4 MB per read, the frame ring, 1000 /
/// ratio) and, for records above 1 M points, what of the record is read for the screen:
/// - the screen holds most of the record: the whole record through the FPGA compression to ~`LIVE_READ` points;
/// - the screen is a small part of it (a fixed depth at a short time base, at the full rate): the screen window with a
///   margin, every sample. Compressed, 10 M at 100 ns/div left about 80 min/max points for the screen.
pub fn batch_plan(s: &Settings, p: &hz::Params, zone_batch: u32) -> Settings {
    let mut b = s.clone();
    if p.memsel > 1_000_000 {
        match super::window::screen_window(s, p) {
            Some(w) => b.window = Some(w),
            None => b.compress = p.memsel.div_ceil(LIVE_READ),
        }
    }
    let fb = oos_hal::dho::seq::frame_bytes(&b, p).max(1);
    // hardware average: one frame is read whatever the batch holds, so the read size does not limit it
    let by_size =
        if matches!(s.mode, oos_hal::dho::acq::Mode::Average { .. }) { u32::MAX } else { (4_000_000 / fb) as u32 };
    b.frames = by_size.min(p.max_frames).min(super::super::BATCH_MAX as u32 / p.ratio.max(1)).max(1);
    if s.mode.is_peak() {
        b.frames = b.frames.min(super::super::PEAK_BATCH_MAX as u32);
    }
    // SINGLE shows the first frame after the press: one frame per batch. A full batch recorded up to 248 more triggers
    // at 1 us/div (`acquired`, unit 2026-11-13) and waited up to 25 ms longer for the ring before the read
    if s.single {
        b.frames = 1;
    }
    // the zone trigger: the ring gets one qualified frame per arm on the unit (round 3: 29 frames/s, the 25 ms wait
    // for more frames per batch), so a batch is `zone_batch` frames (docs/fork/hwzone.md "Rate")
    if s.hwzone.is_some() {
        b.frames = b.frames.min(zone_batch);
    }
    b
}
