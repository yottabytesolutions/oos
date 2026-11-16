// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn capability_and_cross_group_repairs_report_effective_changes_once() {
    for n in 1..=4 {
        for la in [false, true] {
            let caps = crate::config::Config::new(
                n,
                0,
                la,
                [true, true, false, false],
                &[("Auto", 0)],
                [1e9; 3],
                [[1_000_000; 3]; 2],
            );
            let mut m = Model {
                active: 3,
                trig_src: 3,
                trig_src2: 3,
                spec_src: WaveId::Ch(3),
                eye_src: WaveId::Ch(3),
                ..Model::default()
            };
            for ch in &mut m.ch {
                ch.on = true;
                ch.coupling = Coupling::Dc50;
                ch.offset = 1e6;
            }
            m.la.on = true;
            m.zscale = m.tscale + 1;
            m.zdelay = 1e3;
            m.dec_src4 = Some(3);
            m.math_a = 3;
            m.math_b = NCH;
            assert!(normalize(&mut m, &caps));
            assert!(m.ch[..n].iter().all(|ch| ch.on && ch.coupling == Coupling::Dc && ch.offset == ch.offset_max()));
            assert!(m.ch[n..].iter().all(|ch| !ch.on));
            assert!(m.active < n && m.trig_src < n && m.trig_src2 < n);
            assert!(matches!((m.spec_src, m.eye_src), (WaveId::Ch(s), WaveId::Ch(e)) if s < n && e < n));
            assert_eq!(m.math_b, NCH, "math sources remain available");
            assert_eq!(m.la.on, la);
            assert!(m.zscale <= m.tscale);
            assert_eq!(m.zdelay, m.delay);
            assert!(!normalize(&mut m, &caps));
        }
    }
}

#[test]
fn analyzer_sources_follow_an_enabled_channel_before_classification() {
    let mut m = Model::default();
    let mut shadow = Shadow::new(&m, UiAcq::default(), &config(4)).unwrap();
    m.ch[0].on = false;
    m.ch[2].on = true;
    assert!(normalize(&mut m, &config(4)));
    assert_eq!((m.spec_src, m.eye_src), (WaveId::Ch(2), WaveId::Ch(2)));
    let c = shadow.observe(&m, UiAcq::default(), &[], &config(4)).unwrap();
    assert_eq!(c.classes(), Classes::ACQ | Classes::MAPPING | Classes::STATS | Classes::MATH | Classes::VIEW);
    assert!(!normalize(&mut m, &config(4)));
}

#[test]
fn effective_worker_route_is_an_acquisition_input() {
    let m = Model::default();
    let a = UiAcq::of(false, true, false, false, Layout::default());
    let b = UiAcq::of(false, false, false, false, Layout::default());
    assert_eq!(classify(&m, &m, a, a), Classes::NONE);
    assert_eq!(classify(&m, &m, a, b), Classes::ACQ | Classes::STATS);
}

#[test]
fn effective_raw_pair_format_is_an_acquisition_input() {
    let m = Model::default();
    let a = UiAcq::default().with_read_format(4);
    let b = UiAcq::default().with_read_format(1);
    assert_eq!(classify(&m, &m, a, b), Classes::ACQ | Classes::STATS);
    assert_eq!(classify(&m, &m, b, b), Classes::NONE);
}

#[test]
fn live_normalization_reports_only_effective_repair_classes() {
    let mut m = Model::default();
    let mut shadow = Shadow::new(&m, UiAcq::default(), &config(4)).unwrap();
    m.mask_src = 1;
    m.eye_rate += 1.0;
    m.zscale = m.tscale + 1;
    assert_eq!(shadow.normalize_live(&mut m, &config(4)).unwrap(), (true, Classes::MAPPING));
    assert_eq!(shadow.normalize_live(&mut m, &config(4)).unwrap(), (false, Classes::NONE));
    m.mask_src = 3;
    assert_eq!(
        shadow.normalize_live(&mut m, &config(2)).unwrap(),
        (true, Classes::ACQ | Classes::STATS | Classes::MASK | Classes::VIEW),
    );
}
