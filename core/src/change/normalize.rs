// SPDX-License-Identifier: GPL-2.0-or-later

use crate::config::Config;
use crate::model::{Coupling, Model, HDIV, NCH, TIME_SCALES};

/// Fixed group order. Only the reusable shadow scratch uses this point until M2.3a.
pub fn normalize(m: &mut Model, config: &Config) {
    channels(m, config);
    timebase(m);
    trigger(m, config);
    acquire(m, config);
    display(m, config);
    measure(m, config);
    analysis(m, config);
    interfaces(m, config);
}
fn channels(m: &mut Model, config: &Config) {
    for (i, ch) in m.ch.iter_mut().enumerate() {
        if !config.coupling(ch.coupling as usize) {
            ch.coupling = Coupling::Dc;
        }
        if i >= config.channels() {
            ch.on = false;
        }
        let max = ch.offset_max();
        ch.offset = ch.offset.max(-max).min(max);
    }
    if m.active >= config.channels() {
        m.active = 0;
    }
}
fn timebase(m: &mut Model) {
    m.zscale = m.zscale.min(m.tscale);
    let half_main = TIME_SCALES[m.tscale] * m.tfine * HDIV / 2.0;
    let half_zoom = TIME_SCALES[m.zscale] * HDIV / 2.0;
    let lim = (half_main - half_zoom).max(0.0);
    m.zdelay = m.zdelay.max(m.delay - lim).min(m.delay + lim);
}
fn trigger(m: &mut Model, config: &Config) {
    for src in [&mut m.trig_src, &mut m.trig_src2] {
        repair(src, config);
    }
}
fn acquire(m: &mut Model, config: &Config) {
    if !config.la() {
        m.la.on = false;
    }
}
fn display(m: &mut Model, config: &Config) {
    optional(&mut m.z_src, config);
}
fn measure(m: &mut Model, config: &Config) {
    repair(&mut m.dvm_src, config);
    repair(&mut m.cnt_src, config);
}
fn analysis(m: &mut Model, config: &Config) {
    for src in [
        &mut m.dec_src,
        &mut m.dec_src2,
        &mut m.mask_src,
        &mut m.spec_src,
        &mut m.eye_src,
        &mut m.pw_v,
        &mut m.pw_i,
        &mut m.bode_in,
        &mut m.bode_out,
        &mut m.bw_src,
    ]
    .into_iter()
    .chain(m.zones.iter_mut().map(|z| &mut z.src))
    {
        repair(src, config);
    }
    for src in [&mut m.dec_src4, &mut m.dec_diff, &mut m.dec_cs] {
        optional(src, config);
    }
    for src in [&mut m.math_a, &mut m.math_b].into_iter().chain(m.maths.iter_mut().flat_map(|m| [&mut m.a, &mut m.b])) {
        if !config.source_exists(*src, NCH) {
            *src = 0;
        }
    }
    if m.bode_sync >= config.channels() {
        m.bode_sync = config.channels() - 1;
    }
    if m.bode_sw_in > config.channels() {
        m.bode_sw_in = 0;
    }
    if let Some(on) = (0..NCH).find(|&c| m.ch[c].on) {
        for src in [&mut m.spec_src, &mut m.eye_src] {
            if !m.ch[*src].on {
                *src = on;
            }
        }
    }
}
fn interfaces(m: &mut Model, config: &Config) {
    if m.gen_sel >= config.generators().max(1) {
        m.gen_select(0);
    }
    repair(&mut m.afg_arb_src, config);
}
fn repair(src: &mut usize, config: &Config) {
    if *src >= config.channels() {
        *src = 0;
    }
}
fn optional(src: &mut Option<usize>, config: &Config) {
    if src.is_some_and(|c| c >= config.channels()) {
        *src = None;
    }
}
