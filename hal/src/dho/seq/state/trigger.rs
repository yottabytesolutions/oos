// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! TPU trigger programming and holdoff shadow updates.

use super::{Settings, State};
use crate::dho::seq::{time_fs, Trig, HOLDOFF_MIN};
use crate::dho::{tpu, Write};

#[cfg(test)]
mod tests;

/// TPU writes of the trigger in `s` (trigger.md: section 6 edge, section 10 the other types).
pub(super) fn steps(state: &mut State, s: &Settings) -> Vec<Write> {
    let nv = |v: f64| v * 1e9;
    let src = s.trig_src;
    // LA lines have no level: any entry will do
    let tc = if src <= 4 { (src - 1) as usize } else { 0 };
    // the hysteresis comes from the scale of the source at the BNC, and multiplies the scale of each channel
    let noise = tpu::noise_hysteresis(nv(s.ch[tc].scale / s.ch[tc].probe), s.noise_reject);
    let vt: [tpu::Vert; 4] = [0, 1, 2, 3].map(|c| tpu::Vert {
        scale: nv(s.ch[c].scale),
        offset: nv(s.ch[c].offset),
        g: state.gain[c],
        noise,
    });
    let mut w = trigger_writes(s, &mut state.ts, &vt, tc);
    // the shortest holdoff is what every trigger type leaves behind; once changed, it is written until it is back
    let set = (s.holdoff - HOLDOFF_MIN).abs() > 1e-12;
    if set || state.holdoff_set {
        w.extend(tpu::holdoff(&mut state.ts, time_fs(s.holdoff)));
    }
    state.holdoff_set = set;
    w
}

fn edge_writes(s: &Settings, ts: &mut tpu::Shadows, v: &tpu::Vert, level: f64) -> Vec<Write> {
    tpu::edge_coupled(ts, s.trig_src, s.slope, s.trig_couple, level, v.offset, v.scale, v.g, v.noise, 8_000_000)
}

fn trigger_writes(s: &Settings, ts: &mut tpu::Shadows, vt: &[tpu::Vert; 4], tc: usize) -> Vec<Write> {
    let fs = time_fs;
    let nv = |v: f64| v * 1e9;
    let src = s.trig_src;
    let level = nv(s.level);
    let edge = if s.slope == tpu::Slope::Falling { tpu::Edge::Falling } else { tpu::Edge::Rising };
    match &s.trigger {
        Trig::Edge => edge_writes(s, ts, &vt[tc], level),
        Trig::Pulse { pol, when, lower, upper } => tpu::pulse(ts, src, *pol, *when, fs(*lower), fs(*upper), level, vt),
        Trig::Slope { when, lower, upper, lo } => {
            tpu::slope(ts, src, edge, *when, fs(*lower), fs(*upper), level, nv(*lo), vt)
        }
        Trig::Runt { pol, when, lower, upper, lo } => {
            tpu::runt(ts, src, *pol, *when, fs(*lower), fs(*upper), level, nv(*lo), vt, true)
        }
        Trig::Window { event, time, lo } => tpu::window(ts, src, s.slope, *event, fs(*time), level, nv(*lo), vt, true),
        Trig::Timeout { time } => tpu::timeout(ts, src, s.slope, fs(*time), level, vt),
        Trig::Delay { srcb, eb, when, lower, upper, levelb } => {
            tpu::delay(ts, src, edge, *srcb, *eb, *when, fs(*lower), fs(*upper), level, nv(*levelb), vt)
        }
        Trig::Pattern { pats, la, levels } => {
            tpu::pattern(ts, &[&pats[..], &la[..]].concat(), tpu::Logic::And, &levels.map(nv), vt)
        }
        Trig::Duration { pats, la, when, lower, upper, levels } => tpu::duration(
            ts,
            &[&pats[..], &la[..]].concat(),
            tpu::Logic::And,
            *when,
            fs(*lower),
            fs(*upper),
            &levels.map(nv),
            vt,
        ),
        Trig::SetupHold { clk, clk_edge, setup, hold, levelc } => tpu::setup_hold(
            ts,
            *clk,
            src,
            *clk_edge,
            tpu::DataLevel::H,
            tpu::ShEvent::Both,
            fs(*setup),
            fs(*hold),
            nv(*levelc),
            level,
            vt,
        ),
        Trig::Nth { count, idle } => tpu::nth(ts, src, edge, *count, fs(*idle), level, vt),
        Trig::Video { std, sync, line } => {
            tpu::video(ts, src, *std, true, *sync, *line, level, vt, tpu::VideoClk::Fast)
        }
        Trig::Uart(u) => tpu::uart(ts, src, u, level, vt),
        Trig::Spi(x) => tpu::spi(ts, x, level, vt),
        Trig::I2c(x) => tpu::i2c(ts, x, level, vt),
    }
}
