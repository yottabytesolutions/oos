// SPDX-License-Identifier: GPL-2.0-or-later

use crate::config::Config;
use crate::model::{Coupling, Model, HDIV, NCH, TIME_SCALES};

/// Fixed group order. Returns whether any effective value was repaired, for the legacy notification bridge.
pub fn normalize(m: &mut Model, config: &Config) -> bool {
    let mut changed = channels(m, config);
    changed |= timebase(m);
    changed |= trigger(m, config);
    changed |= acquire(m, config);
    changed |= display(m, config);
    changed |= measure(m, config);
    changed |= analysis(m, config);
    changed |= interfaces(m, config);
    changed
}
fn channels(m: &mut Model, config: &Config) -> bool {
    let mut changed = false;
    for (i, ch) in m.ch.iter_mut().enumerate() {
        if !config.coupling(ch.coupling as usize) {
            changed |= set(&mut ch.coupling, Coupling::Dc);
        }
        if i >= config.channels() {
            changed |= set(&mut ch.on, false);
        }
        let max = ch.offset_max();
        let offset = ch.offset.max(-max).min(max);
        changed |= set(&mut ch.offset, offset);
    }
    if m.active >= config.channels() {
        changed |= set(&mut m.active, 0);
    }
    changed
}
fn timebase(m: &mut Model) -> bool {
    let scale = m.zscale.min(m.tscale);
    let mut changed = set(&mut m.zscale, scale);
    let half_main = TIME_SCALES[m.tscale] * m.tfine * HDIV / 2.0;
    let half_zoom = TIME_SCALES[m.zscale] * HDIV / 2.0;
    let lim = (half_main - half_zoom).max(0.0);
    let delay = m.zdelay.max(m.delay - lim).min(m.delay + lim);
    changed |= set(&mut m.zdelay, delay);
    changed
}
fn trigger(m: &mut Model, config: &Config) -> bool {
    let mut changed = repair(&mut m.trig_src, config);
    changed |= repair(&mut m.trig_src2, config);
    changed
}
fn acquire(m: &mut Model, config: &Config) -> bool {
    !config.la() && set(&mut m.la.on, false)
}
fn display(m: &mut Model, config: &Config) -> bool {
    optional(&mut m.z_src, config)
}
fn measure(m: &mut Model, config: &Config) -> bool {
    let mut changed = repair(&mut m.dvm_src, config);
    changed |= repair(&mut m.cnt_src, config);
    changed
}
fn analysis(m: &mut Model, config: &Config) -> bool {
    let mut changed = false;
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
        changed |= repair(src, config);
    }
    for src in [&mut m.dec_src4, &mut m.dec_diff, &mut m.dec_cs] {
        changed |= optional(src, config);
    }
    for src in [&mut m.math_a, &mut m.math_b].into_iter().chain(m.maths.iter_mut().flat_map(|m| [&mut m.a, &mut m.b])) {
        if !config.source_exists(*src, NCH) {
            changed |= set(src, 0);
        }
    }
    if m.bode_sync >= config.channels() {
        changed |= set(&mut m.bode_sync, config.channels() - 1);
    }
    if m.bode_sw_in > config.channels() {
        changed |= set(&mut m.bode_sw_in, 0);
    }
    if let Some(on) = (0..NCH).find(|&c| m.ch[c].on) {
        for src in [&mut m.spec_src, &mut m.eye_src] {
            if !m.ch[*src].on {
                changed |= set(src, on);
            }
        }
    }
    changed
}
fn interfaces(m: &mut Model, config: &Config) -> bool {
    let mut changed = false;
    if m.gen_sel >= config.generators().max(1) {
        m.gen_select(0);
        changed = true;
    }
    changed |= repair(&mut m.afg_arb_src, config);
    changed
}
fn repair(src: &mut usize, config: &Config) -> bool {
    *src >= config.channels() && set(src, 0)
}
fn optional(src: &mut Option<usize>, config: &Config) -> bool {
    src.is_some_and(|c| c >= config.channels()) && set(src, None)
}
fn set<T: PartialEq>(value: &mut T, next: T) -> bool {
    if *value == next {
        return false;
    }
    *value = next;
    true
}
