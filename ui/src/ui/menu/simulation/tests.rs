// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::small_tests::{demo, read_rows, strings};
use super::*;

#[test]
fn every_simulation_row_keeps_kind_label_value_and_options() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.m.sim_rate = 1e9;
    u.m.sim_vhigh = 1.0;
    u.m.sim_vlow = -1.0;
    u.m.sim_rise = 1e-9;
    u.m.sim_bw = 2e9;
    u.m.sim_noise = 0.01;
    u.m.sim_ssc_hz = 30e3;
    u.m.sim_dcd_pp = 2e-12;
    u.m.sim_corr_rj = 3e-12;
    u.m.sim_corr_hz = 4e3;
    u.m.sim_rj = 5e-12;
    u.m.sim_pj = 6e-12;
    u.m.sim_pjhz = 7e3;
    u.m.sim_prbs = 13;
    u.m.sim_deemph = 3.5;
    u.m.sim_ssc = true;
    u.m.sim_ssc_center = false;
    u.m.sim_ssc_deviation = 0.005;
    u.m.sim_slot = 2;
    read_rows(
        &u,
        &[
            (Open(MenuId::Sim), Action, "SIM", "".into(), vec![]),
            (SimLive, Action, "Live", "".into(), vec![]),
            (
                SimPreset,
                Choice,
                "Preset",
                u.sim_preset_name().into(),
                oos_dsp::serial::names().into_iter().map(String::from).collect(),
            ),
            (SimRate, Value, "Rate", "1 Gb/s".into(), vec![]),
            (SimHigh, Value, "High", "1 V".into(), vec![]),
            (SimLow, Value, "Low", "-1 V".into(), vec![]),
            (SimRise, Value, "Rise 20-80%", "1 ns".into(), vec![]),
            (SimBw, Value, "Bandwidth", "2 GHz".into(), vec![]),
            (SimNoise, Value, "Noise", "10 mV".into(), vec![]),
            (SimSscHz, Value, "SSC frequency", "30 kHz".into(), vec![]),
            (SimDcd, Value, "DCD p-p", "2 ps".into(), vec![]),
            (SimCorrRj, Value, "Corr RJ RMS", "3 ps".into(), vec![]),
            (SimCorrHz, Value, "Corr RJ corner", "4 kHz".into(), vec![]),
            (SimRj, Value, "RJ RMS", "5 ps".into(), vec![]),
            (SimPj, Value, "PJ peak", "6 ps".into(), vec![]),
            (
                SimPat,
                Choice,
                "Pattern",
                "PRBS-13".into(),
                strings(&["PRBS-7", "PRBS-9", "PRBS-13", "PRBS-15", "PRBS-23", "PRBS-31"]),
            ),
            (SimDeemph, Value, "De-emph", "3.5 dB".into(), vec![]),
            (SimSsc, Choice, "Spread", "On".into(), strings(&["Off", "On"])),
            (SimSscCenter, Choice, "SSC profile", "Down, DC removed".into(), strings(&["Down, DC removed", "Center"])),
            (SimSscDeviation, Value, "SSC deviation", "0.50 % p-p".into(), vec![]),
            (SimPjHz, Value, "PJ freq", "7 kHz".into(), vec![]),
            (SimSlot, Choice, "Slot", "R3".into(), (1..=NREF).map(|i| format!("R{i}")).collect()),
            (SimPlay, Action, "Play", "".into(), vec![]),
            (SimShow, Choice, "Show", "empty".into(), strings(&["Off", "On"])),
            (SimDefaults, Action, "Defaults", "".into(), vec![]),
            (Open(MenuId::SimAdvanced), Action, "Advanced", "".into(), vec![]),
        ],
    );
}

#[test]
fn simulation_menu_order_and_live_visibility_follow_record_mode() {
    use Item::*;
    let mut u = demo();
    for offline in [false, true] {
        u.offline = offline;
        for sim in [false, true] {
            u.session_is_sim = sim;
            let mut expected = vec![
                SimPreset,
                SimRate,
                SimPat,
                SimHigh,
                SimLow,
                SimRise,
                SimBw,
                SimDeemph,
                SimNoise,
                SimSsc,
                Open(MenuId::SimAdvanced),
                SimRj,
                SimPj,
                SimPjHz,
                SimSlot,
                SimPlay,
                SimShow,
                SimDefaults,
            ];
            if offline || sim {
                expected.insert(0, SimLive);
            }
            assert_eq!(u.menu_items(MenuId::Sim), expected);
            assert_eq!(
                u.menu_items(MenuId::SimAdvanced),
                [SimSsc, SimSscCenter, SimSscDeviation, SimSscHz, SimDcd, SimCorrRj, SimCorrHz, SimDefaults]
            );
        }
    }
}

#[test]
fn simulation_choices_keep_identity_with_decorated_values_and_missing_slots() {
    use Item::*;
    let mut u = demo();
    for (preset, name) in oos_dsp::serial::names().into_iter().enumerate() {
        u.m.sim_preset = preset;
        assert_eq!(u.value(SimPreset), name);
        assert_eq!(active_choice::index(&u, SimPreset), Some(preset));
    }
    for (index, pattern) in [7, 9, 13, 15, 23, 31].into_iter().enumerate() {
        u.m.sim_prbs = pattern;
        assert_eq!(u.value(SimPat), format!("PRBS-{pattern}"));
        assert_eq!(active_choice::index(&u, SimPat), Some(index));
    }
    for on in [false, true] {
        u.m.sim_ssc = on;
        u.m.sim_ssc_center = on;
        u.m.sim_ssc_deviation = 0.0123;
        assert_eq!(u.value(SimSsc), if on { "On" } else { "Off" });
        assert_eq!(u.value(SimSscCenter), if on { "Center" } else { "Down, DC removed" });
        assert_eq!(u.value(SimSscDeviation), if on { "1.23 % pk" } else { "1.23 % p-p" });
        for item in [SimSsc, SimSscCenter] {
            assert_eq!(active_choice::index(&u, item), Some(usize::from(on)));
        }
    }
    u.m.sim_pj = 0.0;
    assert_eq!(u.value(SimPjHz), "off");
    for slot in 0..NREF {
        u.m.sim_slot = slot;
        assert_eq!(u.value(SimSlot), format!("R{}", slot + 1));
        assert_eq!(active_choice::index(&u, SimSlot), Some(slot));
        assert_eq!(u.value(SimShow), "empty");
        assert_eq!(active_choice::index(&u, SimShow), None);
        for label in ["", "Stored capture"] {
            for on in [false, true] {
                u.refs[slot] = Some(RefWave {
                    src: 0,
                    color: 0,
                    label: label.into(),
                    t0: 0.0,
                    dt: 1e-9,
                    v: vec![0.0].into(),
                    vpd: 1.0,
                    offset: 0.0,
                    on,
                });
                let name = if label.is_empty() { format!("R{}", slot + 1) } else { label.into() };
                assert_eq!(u.value(SimShow), format!("{} {}", name, if on { "on" } else { "off" }));
                assert_eq!(active_choice::index(&u, SimShow), Some(usize::from(on)));
            }
        }
    }
}
