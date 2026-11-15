// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn nested_fields_are_exhaustive() {
    let m = Model::default();
    let Channel {
        on: _,
        vscale: _,
        vfine: _,
        unit: _,
        bias: _,
        skew: _,
        offset: _,
        coupling: _,
        bw: _,
        probe: _,
        invert: _,
    } = &m.ch[0];
    let crate::math::MathCh { op: _, a: _, b: _, scale: _, fc: _, bw: _, offset: _, win: _, lin: _ } = &m.maths[0];
    let crate::zone::Zone { on: _, src: _, inside: _, pts: _ } = &m.zones[0];
    let crate::la::LaSet { on: _, show: _, preset: _, user: _, height: _ } = &m.la;
}

#[test]
fn nested_trigger_fields_are_exhaustive() {
    let m = Model::default();
    let GenCh {
        on: _,
        arb: _,
        arb_src: _,
        freq: _,
        ampl: _,
        offset: _,
        wave: _,
        duty: _,
        symmetry: _,
        modulation: _,
        mod_rate: _,
        mod_depth: _,
        fm_dev: _,
        pm_dev: _,
        mod_shape: _,
    } = &m.gen_bank[0];
    let SerialTrig {
        rel: _,
        max: _,
        mask: _,
        uart_inv: _,
        uart_stop: _,
        uart_err: _,
        spi_cs_high: _,
        spi_timeout: _,
        addr_rel: _,
        addr_max: _,
        dir: _,
        ten_bit: _,
        bytes: _,
    } = &m.ser;
    let DecCfg {
        on: _,
        src: _,
        baud: _,
        canrate: _,
        canh: _,
        fdrate: _,
        arate: _,
        tick: _,
        nibbles: _,
        uspeed: _,
        src4: _,
        la_in: _,
        la: _,
        diff: _,
        thr: _,
        ascii: _,
        proto: _,
        src2: _,
        cs: _,
        mode: _,
        bits: _,
        parity: _,
        opt: _,
    } = m.dec_cfg();
}

#[test]
fn every_old_worker_dependency_refreshes_the_worker_key() {
    let mut a = Model::default();
    a.la.on = true;
    a.trig_la = Some(0);
    let needs = Layout::default();
    let key = WorkerViewKey::of(&a, needs, &config(4));
    let setters: &[fn(&mut Model)] = &[
        |m| m.ch[0].on = !m.ch[0].on,
        |m| m.ch[0].unit = AmpUnit::A,
        |m| m.ch[0].vscale += 1,
        |m| m.ch[0].vfine += 0.1,
        |m| m.ch[0].bias += 0.1,
        |m| m.ch[0].skew += 1e-9,
        |m| m.ch[0].offset += 0.1,
        |m| m.ch[0].coupling = Coupling::Ac,
        |m| m.ch[0].bw += 1,
        |m| m.ch[0].probe *= 10.0,
        |m| m.ch[0].invert = !m.ch[0].invert,
        |m| m.tscale += 1,
        |m| m.delay += 1e-3,
        |m| m.trig_src = 1,
        |m| m.trig_la = Some(1),
        |m| m.trig_type = TrigType::Pulse,
        |m| m.slope = Slope::Fall,
        |m| m.trig_level += 0.2,
        |m| m.trig_coup = TrigCoup::Ac,
        |m| m.trig_nrej = !m.trig_nrej,
        |m| m.la.on = false,
        |m| m.acq = AcqMode::Peak,
        |m| m.dots = DotsMode::On,
    ];
    for (index, set) in setters.iter().enumerate() {
        let mut b = a.clone();
        set(&mut b);
        // LA off without an active LA trigger changes no worker dependency.

        assert_ne!(key, WorkerViewKey::of(&b, needs, &config(4)));
        if index < 11 {
            let expected = if index == 1 { Classes::NONE } else { Classes::ACQ | Classes::MAPPING | Classes::STATS };
            assert_eq!(classify(&a, &b, UiAcq::default(), UiAcq::default()), expected);
        }
    }
    let mut b = a.clone();
    b.trig_level += 0.2;
    assert_eq!(MappingKey::of(&a, needs), MappingKey::of(&b, needs));
    assert_ne!(WorkerViewKey::of(&a, needs, &config(4)), WorkerViewKey::of(&b, needs, &config(4)));
}

#[test]
fn nested_logic_fields_have_exact_contracts() {
    let a = Model::default();
    type LogicCase = (fn(&mut Model), Classes);
    let cases: &[LogicCase] = &[
        (|m| m.la.on = !m.la.on, Classes::ACQ | Classes::STATS),
        (|m| m.la.show ^= 1, Classes::VIEW),
        (|m| m.la.preset[0] += 1, Classes::ACQ | Classes::STATS),
        (|m| m.la.user[0] += 1.0, Classes::ACQ | Classes::STATS),
        (|m| m.la.height += 1, Classes::VIEW),
    ];
    for (edit, expected) in cases {
        let mut b = a.clone();
        edit(&mut b);
        assert_eq!(classify(&a, &b, UiAcq::default(), UiAcq::default()), *expected);
    }
}
