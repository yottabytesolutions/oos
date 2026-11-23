// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{
    board::{Frame, PictureEligibility},
    model::*,
    picture::{
        deposit::{Deposit, TapProjection},
        DepositWindow,
    },
    sparse::Interp,
    wave::Phosphor,
};
use oos_hal::dho::{hz, seq::Settings};
use std::any::Any;
use std::sync::{atomic::Ordering, Arc, Mutex};
use worker_shutdown::one_worker_held;

mod deposit;
mod perf;
mod publication;
mod routing;
mod window;
mod worker_shutdown;

/// A read-out buffer for the tests.
pub(super) struct Bytes(pub(super) Vec<u8>);

impl RawBatch for Bytes {
    fn bytes(&self) -> &[u8] {
        &self.0
    }
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send> {
        self
    }
}

fn fresh() -> &'static Counters {
    Box::leak(Box::default())
}

pub(super) fn dec(s: &Settings, p: &hz::Params) -> DecodeInto {
    let (s, p) = (s.clone(), *p);
    Arc::new(move |c: &[u8], f: &mut Frame| crate::board::dho::hw::decode_into(c, &s, &p, 0, f))
}

/// A raw batch of `frames` frames, CH1 (and CH2 with `two`): a sine with a rising zero crossing near the hardware
/// trigger point, a per-frame jitter, every fifth frame shifted up, every seventh without the crossing.
pub(super) fn batch(frames: usize, memsel: u32, two: bool) -> (Vec<u8>, usize, Settings, hz::Params) {
    let r = if two { 2 } else { 1 };
    let p = hz::Params {
        ratio: r,
        rate: 1_250_000_000 / r as u64,
        memsel,
        dot_fs: 800_000,
        seg: 0,
        max_frames: 1000,
        pre: memsel / 2,
        post: memsel / 2,
    };
    let s = Settings { mask: if two { 3 } else { 1 }, window: None, compress: 1, header: false, ..Settings::default() };
    let fb = oos_hal::dho::seq::frame_bytes(&s, &p);
    let mut bytes = Vec::new();
    for k in 0..frames {
        let jit = (k % 7) as f64 * 0.13 - 0.4;
        for i in 0..memsel as usize {
            let ph = (i as f64 - memsel as f64 / 2.0 + jit) / 400.0 * std::f64::consts::TAU;
            for c in 0..r as usize {
                let v = if k % 7 == 6 {
                    0.7
                } else {
                    0.9 * (ph * (1.0 + c as f64 * 0.5)).sin() + if k % 5 == 0 { 0.05 } else { 0.0 }
                };
                bytes.extend(((32768.0 + v * 7500.0) as u16).to_le_bytes());
            }
        }
    }
    assert_eq!(bytes.len(), frames * fb, "frame layout");
    (bytes, fb, s, p)
}

pub(super) fn model(tdiv: f64, two: bool, dots: bool) -> Model {
    let mut m = Model {
        tscale: tscales().iter().position(|&t| (t as f64 - tdiv).abs() < tdiv * 1e-3).unwrap(),
        trig_level: 0.0,
        dots: if dots { DotsMode::On } else { DotsMode::Off },
        ..Model::default()
    };
    for c in 0..NCH {
        (m.ch[c].on, m.ch[c].offset) = (c == 0 || c == 1 && two, 0.0);
    }
    m
}

/// Frames of channels `mask` (ratio from the mask), `n` points each: per channel a signal from `sig(frame, channel,
/// point)` in volts at 1 V/div (7500 codes per volt).
fn frames_of(
    mask: u32,
    n: u32,
    frames: usize,
    sig: &dyn Fn(usize, usize, usize) -> f64,
) -> (Vec<u8>, usize, Settings, hz::Params) {
    let r = hz::ratio(mask);
    let p = hz::Params {
        ratio: r,
        rate: 1_250_000_000 / r as u64,
        memsel: n,
        dot_fs: 800_000 * r as u64,
        seg: 0,
        max_frames: 1000,
        pre: n / 2,
        post: n / 2,
    };
    let s = Settings { mask, window: None, compress: 1, header: false, ..Settings::default() };
    let fb = oos_hal::dho::seq::frame_bytes(&s, &p);
    let map = oos_hal::dho::seq::code_map(mask, s.ch.map(|k| (k.scale, k.offset)));
    let mut bytes = vec![0u8; frames * fb];
    for k in 0..frames {
        for i in 0..n as usize {
            for c in 0..NCH {
                if let Some(wd) = map.word[c] {
                    let x = (32768.0 + sig(k, c, i) * 7500.0).round().clamp(0.0, 65535.0) as u16;
                    let j = k * fb + 2 * (i * r as usize + wd);
                    bytes[j..j + 2].copy_from_slice(&x.to_le_bytes());
                }
            }
        }
    }
    (bytes, fb, s, p)
}

/// Until every batch fed is done (drawn and handed on, or dropped).
fn settle(p: &Pipe) {
    let t0 = crate::ui::lifecycle::test_wait::Wait::new("all submitted deposit batches completed");
    while p.live.load(Ordering::Acquire) > 0 {
        t0.check();
        t0.poll();
    }
}

fn collect(pipe: &Pipe, ph: &mut Phosphor, clear: &mut bool, edges: &mut [u64; 4], off: &mut (f64, f64)) -> Accepted {
    let mut total = Accepted::default();
    loop {
        host::barrier(pipe);
        let next = pipe.accept(ph, clear, edges, off);
        if next.frames == 0 {
            return total;
        }
        total.frames += next.frames;
        total.triggered += next.triggered;
        total.newest = total.newest.max(next.newest);
        total.stale += next.stale;
        crate::framestats::add_folds(&mut total.folds, &next.folds);
    }
}

/// Buffer return follows publication and the live-count decrement. Await that event directly so timed
/// rounds include worker completion without the ordinary test counter poll's sleep granularity.
/// Independent reference using direct drawing and statistics on every decoded frame.
fn reference(
    bytes: &[u8],
    fb: usize,
    s: &Settings,
    p: &hz::Params,
    m: &Model,
    w: usize,
    h: usize,
) -> (Phosphor, u64, [u64; 4], (f64, f64), crate::framestats::Folds) {
    let mut want = Phosphor::new(w, h);
    let (mut trig, mut edges, mut off) = (0, [0u64; 4], (0.0, 0.0));
    let mut folds = crate::framestats::folds_for(m);
    for chunk in bytes.chunks_exact(fb) {
        let f = crate::board::dho::hw::decode(chunk, s, p, 0);
        trig += draw_frame(&mut want, m, &f, Interp::Exact, Some(Stats { edges: &mut edges, off: &mut off })) as u64;
        crate::framestats::fold_frame(&f, m, &mut folds);
    }
    (want, trig, edges, off, folds)
}

fn recycled(p: &Pipe) -> Box<Bytes> {
    oos_hal::sync::lock(&p.pool)
        .recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD)
        .expect("deposit buffer returned to pool after all workers completed")
        .into_any()
        .downcast::<Bytes>()
        .expect("the benchmark feeds Bytes buffers")
}
