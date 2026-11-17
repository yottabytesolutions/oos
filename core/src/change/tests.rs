// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::model::*;

pub(super) fn config(channels: usize) -> crate::config::Config {
    crate::config::Config::new(
        channels,
        1,
        true,
        [true, true, true, false],
        &[("Auto", 0), ("100M", 100_000_000)],
        [1.25e9, 625e6, 312.5e6],
        [[125_000_000; 3]; 2],
    )
}

#[test]
fn every_field_has_a_group() {
    let m = Model::default();
    flat::assert_exhaustive(&m);
    let mut indexes = Vec::new();
    fields(&m, &m, |f| {
        if !MEMBER_FIELDS.contains(&f.index()) {
            indexes.push(f.index());
        }
    });
    indexes.sort_unstable();
    assert_eq!(indexes, (0..MEMBER_FIELDS.start).chain(MEMBER_FIELDS.end..FIELD_COUNT).collect::<Vec<_>>());
}

#[test]
fn unrelated_fields_snapshot() {
    assert_eq!(
        contracts::unrelated(),
        &[
            "active",
            "auto_nrej",
            "bar_compact",
            "bus",
            "ch_label",
            "cur_a",
            "cur_b",
            "cur_b_source",
            "cur_c",
            "cur_d",
            "cursor",
            "dec_ascii",
            "dec_sel",
            "decs",
            "fine_t",
            "fine_v",
            "grid",
            "grid_bright",
            "grid_frame",
            "h_expand",
            "hdmi",
            "hdmi_view",
            "hist",
            "intensity",
            "log_ivl",
            "mag_cur",
            "mask_stop",
            "math_sel",
            "meas_labels",
            "quick",
            "spec_harm",
            "spec_sort",
            "text_size",
            "touch_lock",
            "v_expand_gnd",
            "view_cur",
            "view_slots",
            "views",
        ]
    );
}

#[test]
fn normalize_is_idempotent() {
    for n in 1..=4 {
        let mut m = Model { active: 3, ..Model::default() };
        m.ch[3].on = true;
        m.ch[0].offset = 1e6;
        m.ch[0].coupling = Coupling::Dc50;
        m.trig_src = 3;
        m.math_a = 3;
        m.dec_src4 = Some(3);
        m.spec_src = WaveId::Ch(3);
        m.eye_src = WaveId::Ch(3);
        m.zscale = m.tscale + 1;
        m.zdelay = 1e3;
        normalize(&mut m, &config(n));
        let first = m.clone();
        assert!(!normalize(&mut m, &config(n)));
        assert_eq!(m, first);
        assert_eq!(m.ch[0].coupling, Coupling::Dc);
        assert!(m.active < n && m.trig_src < n);
    }
}

#[test]
fn missing_channel_is_acq() {
    let mut m = Model::default();
    m.ch[3].on = true;
    m.trig_src2 = 0;
    m.dec_src = 0;
    m.dec_src2 = 0;
    m.bode_sync = 0;
    // the analyser defaults that name CH3/CH4 stay out of this case: their repair is VIEW
    (m.inv_c, m.inv_an, m.dpt_g, m.dpt_4ch) = (WaveId::Ch(0), WaveId::Ch(0), WaveId::Ch(0), WaveId::Ch(0));
    let mut s = Shadow::new(&m, UiAcq::default(), &config(4)).unwrap();
    let c = s.observe(&m, UiAcq::default(), &[], &config(2)).unwrap();
    assert_eq!(c.classes(), Classes::ACQ | Classes::MAPPING | Classes::STATS);
    assert!(!s.normalized().ch[3].on);
}

#[test]
fn clamped_offset_is_mapping() {
    let m = Model::default();
    let mut s = Shadow::new(&m, UiAcq::default(), &config(4)).unwrap();
    let mut large = m.clone();
    large.ch[0].offset = 1e6;
    let c = s.observe(&large, UiAcq::default(), &[], &config(4)).unwrap();
    assert_eq!(c.classes(), Classes::ACQ | Classes::MAPPING | Classes::STATS);
    assert_eq!(s.normalized().ch[0].offset, large.ch[0].offset_max());
    assert_eq!(m.ch[0].offset, Model::default().ch[0].offset);
}

#[test]
fn change_values_stay_bounded() {
    use std::mem::size_of;
    assert_eq!(size_of::<Generations>(), 24);
    assert!(size_of::<ChangeSet>() < 4096);
    assert!(size_of::<UiAcq>() < 1024);
    assert!(size_of::<DepositView>() < 1024);
    assert!(size_of::<AcqIntent<'_>>() < 4096);
    assert!(Outbox::default().bytes() < 64 * 1024);
    let m = Model { bode_addr: "x".repeat(64 * 1024), ..Model::default() };
    assert!(matches!(Shadow::new(&m, UiAcq::default(), &config(4)), Err(Error::SnapshotTooLarge)));
}

mod contracts;
mod nested;
mod normalization;
mod requests;

mod table;

#[test]
fn allocation_free_time_ladder_is_bit_identical_to_the_legacy_ladder() {
    let mut legacy = Vec::new();
    let mut decade = 1e-11_f64;
    while decade < 2000.0 {
        for multiplier in [1.0, 2.0, 5.0] {
            let value = decade * multiplier;
            if (0.49e-10..=1000.1).contains(&value) {
                legacy.push(value as f32);
            }
        }
        decade *= 10.0;
    }
    assert_eq!(legacy.len(), TIME_SCALES.len());
    for (index, expected) in legacy.into_iter().enumerate() {
        assert_eq!(TIME_SCALES[index].to_bits(), expected.to_bits());
        let model = Model { tscale: index, ..Model::default() };
        assert_eq!(model.time_per_div().to_bits(), expected.to_bits());
    }
}

#[test]
fn shadow_normalize_does_not_panic_on_nonfinite_bounds() {
    for delay in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut m = Model { delay, zdelay: 1.0, ..Model::default() };
        m.ch[0].probe = f32::NAN;
        m.ch[0].offset = 1.0;
        normalize(&mut m, &config(4));
        assert_eq!(m.ch[0].offset, 1.0);
    }
}
