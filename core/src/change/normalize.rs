// SPDX-License-Identifier: GPL-2.0-or-later

use crate::config::Config;
use crate::model::{decode_wave, Coupling, Model, WaveId, HDIV, MEAS_SELECTION_MAX, NCH, NREF, TIME_SCALES};

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
    // a two-level type keeps its lower level at or below the upper one; the lower gives way, since the upper is the
    // level every type shares and the lower is also Delay's level B and Setup/Hold's clock level
    if m.trig_type.two_level() && m.trig_level2 > m.trig_level {
        changed |= set(&mut m.trig_level2, m.trig_level);
    }
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
    for row in &mut m.meas {
        changed |= wave(&mut row.1, config);
    }
    for row in &mut m.meas2 {
        changed |= wave(&mut row.1, config);
        changed |= wave(&mut row.2, config);
    }
    let rows = m.cat.len();
    m.cat.retain(|row| config.catalog_ok(row.0));
    m.cat.truncate(MEAS_SELECTION_MAX.saturating_sub(m.meas.len() + m.meas2.len()));
    changed |= rows != m.cat.len();
    for row in &mut m.cat {
        for source in [&mut row.1, &mut row.2, &mut row.3, &mut row.4] {
            changed |= wave(source, config);
            if matches!(*source, WaveId::Ref(i) if i >= NREF) {
                changed |= set(source, WaveId::Ref(0));
            }
        }
    }
    changed
}
fn analysis(m: &mut Model, config: &Config) -> bool {
    let mut changed = false;
    for src in [&mut m.mask_src, &mut m.bw_src].into_iter().chain(m.zones.iter_mut().map(|z| &mut z.src)) {
        changed |= repair(src, config);
    }
    // decoder sources: channels, then refs from `DEC_REF_BASE` (`decode_wave`)
    for src in [&mut m.dec_src, &mut m.dec_src2] {
        if decode_wave(*src).is_none() || (*src < NCH && *src >= config.channels()) {
            changed |= set(src, 0);
        }
    }
    for src in [&mut m.dec_src4, &mut m.dec_diff, &mut m.dec_cs] {
        if src.is_some_and(|c| decode_wave(c).is_none() || (c < NCH && c >= config.channels())) {
            changed |= set(src, None);
        }
    }
    changed |= analyser_sources(m, config);
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
            if matches!(*src, WaveId::Ch(c) if !m.ch[c].on) {
                changed |= set(src, WaveId::Ch(on));
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
/// The analysers' sources: a channel the unit lacks becomes CH1 (or none where optional); refs stay.
fn analyser_sources(m: &mut Model, config: &Config) -> bool {
    let mut changed = false;
    for src in [
        &mut m.spec_src,
        &mut m.eye_src,
        &mut m.pw_v,
        &mut m.pw_i,
        &mut m.bode_in,
        &mut m.bode_out,
        &mut m.mag_v,
        &mut m.mag_i,
        &mut m.inv_a,
        &mut m.inv_b,
        &mut m.inv_c,
        &mut m.inv_an,
        &mut m.dpt_v,
        &mut m.dpt_i,
        &mut m.dpt_g,
        &mut m.dpt_4ch,
    ] {
        changed |= wave(src, config);
    }
    for src in
        [&mut m.inv_tq, &mut m.inv_ev, &mut m.inv_ei, &mut m.eye_b, &mut m.eye_rail, &mut m.pw_vin, &mut m.pw_clk]
    {
        if matches!(*src, Some(WaveId::Ch(c)) if c >= config.channels()) {
            changed |= set(src, None);
        }
    }
    changed
}
/// A channel the unit lacks becomes CH1; refs stay.
fn wave(src: &mut WaveId, config: &Config) -> bool {
    matches!(*src, WaveId::Ch(c) if c >= config.channels()) && set(src, WaveId::Ch(0))
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
