// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Hardware mask and zone qualification sequences.

use super::{Settings, State, Step};
use crate::dho::{hz, mask, zone};

#[cfg(test)]
mod tests;

/// The mask unit for `s` (docs/fork/hwmask.md): nothing while it runs as wanted; else off, the table when it
/// changed, compression, slot (counters cleared on a new slot, a new start or a clear request) and on again; off
/// when it is not wanted or its channel is not acquired. Called in the acquisition block with the FSM stopped.
pub(super) fn mask_steps(state: &mut State, s: &Settings, p: &hz::Params) -> Vec<Step> {
    let want = s.hwmask.as_ref().and_then(|m| mask::slot(s.mask, m.ch).map(|sl| (m, sl)));
    let Some((m, slot)) = want else {
        if state.mask_run.take().is_none() {
            return vec![];
        }
        state.sh.s4020 = mask::scu_bits(state.sh.s4020, false, false);
        let mut w = vec![Step::Note("mask test off".into()), Step::Wr((0x4020, state.sh.s4020))];
        // the zone trigger keeps the slot and the strobe it set up
        if state.zone_run.is_none() {
            state.s1190 &= !mask::ctrl::SLOT_MASK;
            w.extend([Step::Wr((mask::COMPRESS, mask::compress::OFF)), Step::Wr((mask::CTRL, state.s1190))]);
        }
        return w;
    };
    let k = &s.ch[m.ch];
    // the software mask spreads over the screen: 10 divisions around the delay
    let t = mask::Timing {
        points: p.memsel,
        pre: p.pre,
        rate: p.rate,
        mask: s.mask,
        screen: (s.delay - 5.0 * s.tdiv, 10.0 * s.tdiv),
    };
    let cols = mask::table_map(&m.lo, &m.hi, &t, k.scale, k.offset, state.gain[m.ch], &m.map);
    let run = (slot, m.map.layout(p.memsel, s.mask).1, m.clear, m.stop_on_fail);
    if state.mask_run == Some(run) && state.mask_cols.as_ref() == Some(&cols) {
        return vec![];
    }
    let clear = state.mask_run.is_none_or(|r| r.0 != run.0 || r.2 != run.2);
    let mut w = vec![];
    state.sh.s4020 = mask::scu_bits(state.sh.s4020, false, false);
    w.push((0x4020, state.sh.s4020));
    if state.mask_cols.as_ref() != Some(&cols) {
        w.extend(mask::load(&cols));
        state.mask_cols = Some(cols);
    }
    w.push((mask::COMPRESS, mask::compress_word(run.1)));
    w.extend(mask::select(&mut state.s1190, slot, clear));
    state.sh.s4020 = mask::scu_bits(state.sh.s4020, true, m.stop_on_fail);
    w.push((0x4020, state.sh.s4020));
    state.mask_run = Some(run);
    let mut st = vec![Step::Note(format!(
        "mask test CH{} slot {}, {} samples per column{}",
        m.ch + 1,
        slot,
        run.1,
        if clear { ", counters cleared" } else { "" }
    ))];
    st.extend(w.into_iter().map(Step::Wr));
    st
}

/// The zone unit for `s` (docs/fork/hwzone.md), like `mask_steps`: nothing while it runs as wanted; else off, the
/// tables that changed, control, compression, slot (the counter cleared on a new slot, a new start or a clear
/// request) and armed; off when it is not wanted, the mask test is wanted, or its channel is not acquired.
pub(super) fn zone_steps(state: &mut State, s: &Settings, p: &hz::Params) -> Vec<Step> {
    let want = s
        .hwzone
        .as_ref()
        .filter(|z| s.hwmask.is_none() && !z.zones.is_empty())
        .and_then(|z| mask::slot(s.mask, z.ch).map(|sl| (z, sl)));
    let Some((z, slot)) = want else {
        if state.zone_run.take().is_none() {
            return vec![];
        }
        state.sh.s4020 = zone::scu_bit(state.sh.s4020, false);
        let mut w =
            vec![Step::Note("zone trigger off".into()), Step::Wr((0x4020, state.sh.s4020)), Step::Wr((zone::CTRL, 0))];
        if state.mask_run.is_none() {
            state.s1190 &= !mask::ctrl::SLOT_MASK;
            w.extend([Step::Wr((mask::COMPRESS, mask::compress::OFF)), Step::Wr((mask::CTRL, state.s1190))]);
        }
        return w;
    };
    let k = &s.ch[z.ch];
    let tabs: Vec<Vec<zone::Column>> = z
        .zones
        .iter()
        .take(2)
        .map(|x| zone::table(x, p.memsel, p.rate, p.pre, s.mask, k.scale, k.offset, state.gain[z.ch], &z.map))
        .collect();
    let n = z.map.layout(p.memsel, s.mask).1;
    let run = (slot, n, zone::ctrl_word(&z.zones), z.clear);
    if state.zone_run == Some(run) && tabs.iter().enumerate().all(|(i, t)| state.zone_tabs[i].as_ref() == Some(t)) {
        return vec![];
    }
    let clear = state.zone_run.is_none_or(|r| r.0 != run.0 || r.3 != run.3);
    let mut w = vec![];
    state.sh.s4020 = zone::scu_bit(state.sh.s4020, false);
    w.push((0x4020, state.sh.s4020));
    for (i, t) in tabs.into_iter().enumerate() {
        if state.zone_tabs[i].as_ref() != Some(&t) {
            w.extend(zone::load(i, &t));
            state.zone_tabs[i] = Some(t);
        }
    }
    w.push((zone::CTRL, run.2));
    w.push((mask::COMPRESS, mask::compress_word(n)));
    w.extend(mask::select(&mut state.s1190, slot, clear));
    state.sh.s4020 = zone::scu_bit(state.sh.s4020, true);
    w.push((0x4020, state.sh.s4020));
    state.zone_run = Some(run);
    let mut st = vec![Step::Note(format!(
        "zone trigger CH{} slot {}, control {:#x}{}",
        z.ch + 1,
        slot,
        run.2,
        if clear { ", counter cleared" } else { "" }
    ))];
    st.extend(w.into_iter().map(Step::Wr));
    st
}
