// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
mod legacy;

pub(super) fn populated() -> Model {
    let mut m = super::tests::custom();
    m.cur_b_source = Some(WaveId::Ref(2));
    m.cat = vec![(90, WaveId::Ch(0), WaveId::Ref(1), WaveId::Ch(2), WaveId::Ch(3))];
    m.tfine = 0.5;
    for i in 0..NCH {
        m.deskew_ch[i] = (i + 1) as f64 * 1e-9;
        m.ch[i].vfine = 1.25;
        m.ch[i].unit = AmpUnit::A;
        m.ch[i].bias = 0.25;
        m.ch[i].skew = 1e-9;
        m.ch_label[i] = format!("probe{i}");
    }
    for i in 0..NREF {
        m.deskew_ref[i] = -(i as f64 + 1.0) * 1e-9;
    }
    for i in 0..crate::math::NMATH {
        m.math_expr[i] = format!("CH{}+1", i % NCH + 1);
        if i != m.math_sel {
            m.maths[i].fc = 100.0 + i as f32;
            m.maths[i].offset = i as f32;
        }
    }
    for i in 0..DECODERS {
        if i != m.dec_sel {
            let mut decoder = m.dec_view(i);
            decoder.dec_on = true;
            decoder.dec_thr = i as f32 * 0.5;
            m.dec_store(i, decoder.dec_cfg());
        }
    }
    for slot in &mut m.view_slots {
        *slot = "cursor=Time; zoom=false".into();
    }
    for zone in &mut m.zones {
        zone.on = true;
        zone.pts = vec![(0.0, -0.5), (1e-6, 0.75)];
    }
    m
}

fn randomized(seed: u64) -> Model {
    let mut state = seed;
    let mut next = |n: usize| {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (state >> 32) as usize % n
    };
    let mut m = populated();
    m.active = next(NCH);
    m.tscale = next(tscales().len());
    m.zscale = next(m.tscale + 1);
    m.trig_src = next(NCH);
    m.trig_level = next(1000) as f32 / 32.0 - 16.0;
    m.delay = next(1000) as f32 * 1e-9;
    m.intensity = 10 + next(91) as u32;
    m.grid_bright = 10 + next(91) as u32;
    m.avg_count = 1 + next(512) as u32;
    m.cur_a = next(1000) as f32 / 1000.0;
    m.cur_b = next(1000) as f32 / 1000.0;
    m.math_fc = next(100_000) as f32;
    m.math_offset = next(1000) as f32 / 64.0;
    m.bus.pos = BUS_POS_MIN + (BUS_POS_MAX - BUS_POS_MIN) * next(1000) as f32 / 1000.0;
    m.sim_ssc_deviation = next(500) as f64 * 1e-5;
    for (i, ch) in m.ch.iter_mut().enumerate() {
        ch.on = next(2) == 0;
        ch.vscale = next(VSCALES.len());
        ch.vfine = 1.0 + next(96) as f32 / 64.0;
        ch.offset = next(1000) as f32 / 128.0;
        ch.invert = next(2) == 0;
        m.deskew_ch[i] = next(1000) as f64 * 1e-12;
    }
    m
}

fn fixtures() -> Vec<Model> {
    let mut models = vec![
        Model::default(),
        super::tests::custom(),
        populated(),
        parse_setup(include_str!("../../../fuzz/corpus/setup/settings")).unwrap(),
    ];
    models.extend((1..=16).map(randomized));
    models
}

#[test]
fn writer_matches_legacy_bytes_for_fixtures_and_randomized_models() {
    for (i, model) in fixtures().iter().enumerate() {
        assert_eq!(setup_text(model).as_bytes(), legacy::setup_text(model).as_bytes(), "fixture {i}");
    }
}

#[test]
fn every_emitted_key_round_trips_into_the_same_model() {
    for (i, model) in fixtures().iter().enumerate() {
        let mut restored = Model::default();
        for line in legacy::setup_text(model).lines().skip(1) {
            let (key, value) = line.split_once(" = ").unwrap();
            set(&mut restored, key, value).unwrap_or_else(|e| panic!("fixture {i}, {key}: {e}"));
        }
        assert_eq!(&restored, model, "fixture {i}");
        assert_eq!(&parse_setup(&setup_text(model)).unwrap(), model, "fixture {i}");
    }
}
