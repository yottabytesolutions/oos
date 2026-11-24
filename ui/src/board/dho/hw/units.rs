// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! DVM and frequency counter service on the acquisition thread.

#[cfg(target_os = "linux")]
use crate::board::HwShared;
#[cfg(target_os = "linux")]
use oos_hal::dho::seq::Settings;

/// Seconds between two readings of the DVM block, and from its (re)start to the first one. The window of 2^28 samples
/// is 0.21 s at 1.25 GSa/s; how long it really takes is open (docs/fork/dvm.md).
const DVM_READ_S: f64 = 0.5;

/// What the DVM block runs on: channel, lane, and the number of the acquisition configuration it was started after.
pub(super) type DvmKey = (usize, u32, u64);

/// The next step of the DVM block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DvmStep {
    Idle,
    /// (re)start on this lane
    Start(u32),
    Stop,
    Read,
}

/// The DVM block's next step: (re)started when the wanted channel, its lane in the acquired set `mask` or the
/// acquisition configuration (`config`, raised on every configuration) changed; stopped when nothing is wanted or the
/// channel is not acquired; read `DVM_READ_S` after the start or the last reading.
pub(super) fn dvm_step(
    want: Option<crate::board::DvmWant>,
    mask: u32,
    config: u64,
    running: Option<DvmKey>,
    elapsed_s: f64,
) -> (DvmStep, Option<DvmKey>) {
    let target = want.and_then(|w| oos_hal::dho::dvm::lane_forced(mask, w.ch, w.lane).map(|l| (w.ch, l, config)));
    if target != running {
        let step = match (target, running) {
            (Some((_, l, _)), _) => DvmStep::Start(l),
            (None, Some(_)) => DvmStep::Stop,
            (None, None) => DvmStep::Idle,
        };
        return (step, target);
    }
    (if running.is_some() && elapsed_s >= DVM_READ_S { DvmStep::Read } else { DvmStep::Idle }, running)
}

/// One reading of the DVM block for the UI: the raw registers and, when the decode takes them, the three values in the
/// volts of channel `ch` of `s`.
#[cfg(target_os = "linux")]
pub(super) fn dvm_reading(
    raw: &oos_hal::dho::dvm::Raw,
    key: DvmKey,
    s: &Settings,
    gain: &[f64; 4],
    n: u64,
) -> crate::board::DvmRead {
    use oos_hal::dho::dvm;
    let (ch, lane, _) = key;
    let vert = oos_hal::dho::levels::Vert { scale: s.ch[ch].scale, offset: s.ch[ch].offset, gain: gain[ch] };
    let volts = dvm::decode(raw, dvm::EXP)
        .map(|r| [dvm::Mode::Dc, dvm::Mode::AcRms, dvm::Mode::AcDcRms].map(|m| dvm::volts(&r, m, &vert)));
    crate::board::DvmRead {
        ch,
        lane,
        exp: dvm::EXP,
        raw: [raw.cfg, raw.sum_lo, raw.sum_hi, raw.sq_lo, raw.sq_hi],
        volts,
        n,
    }
}
/// read once a gate (and 0.1 s) has passed since the start or the last reading (docs/protocols/counter.md).
#[cfg(target_os = "linux")]
pub(super) fn counter_tick(
    bar: &mut oos_hal::dho::bus::Bar1,
    out: &HwShared,
    want: Option<crate::board::CounterWant>,
    counting: &mut Option<(crate::board::CounterWant, std::time::Instant)>,
    n: &mut u64,
) {
    use oos_hal::dho::bus::Bus;
    use oos_hal::dho::counter;
    if want != counting.map(|c| c.0) {
        match want {
            Some(w) => {
                let mode = [counter::Mode::Frequency, counter::Mode::Period, counter::Mode::Totalize][w.mode.min(2)];
                bar.apply(&counter::start(&counter::Config { ch: w.ch, mode, digits: w.digits }));
                *counting = Some((w, std::time::Instant::now()));
            }
            None => {
                bar.apply(&counter::stop());
                *counting = None;
            }
        }
        crate::perfguard::deep::acq_lock(out).counter = None;
        return;
    }
    if let Some((w, t)) = *counting {
        let gate = counter::gate_s(w.digits);
        if t.elapsed().as_secs_f64() >= gate + 0.1 {
            let r = counter::read(bar);
            *n += 1;
            crate::perfguard::deep::acq_lock(out).counter =
                Some(crate::board::CounterRead { period: counter::period(&r, gate), total: r.total, n: *n });
            *counting = Some((w, std::time::Instant::now()));
        }
    }
}
