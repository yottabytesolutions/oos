// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::small_tests::{demo, read_rows, strings};
use super::*;

#[test]
fn every_inverter_row_keeps_kind_label_value_and_options() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.m.inv_a = WaveId::Ch(0);
    u.m.inv_b = WaveId::Ch(1);
    u.m.inv_c = WaveId::Ch(2);
    u.m.inv_an = WaveId::Ch(3);
    u.m.inv_ang = InvAngle::Analog;
    u.m.inv_page = Page::Dq0;
    u.m.inv_volt = true;
    u.m.inv_poles = 3;
    u.m.inv_scale = 12.0;
    u.m.inv_degv = 2.5;
    u.m.inv_off = -5.0;
    u.m.inv_elec = true;
    u.m.inv_qa = 2;
    u.m.inv_qb = 3;
    u.m.inv_qi = None;
    u.m.inv_cpr = 1234.0;
    u.m.inv_swap = true;
    (u.m.inv_ha, u.m.inv_hb, u.m.inv_hc) = (4, 5, 6);
    (u.m.inv_tq, u.m.inv_ev, u.m.inv_ei) = (None, Some(WaveId::Ch(0)), Some(WaveId::Ch(1)));
    read_rows(
        &u,
        &[
            (Open(MenuId::Inverter), Action, "Inverter", "".into(), vec![]),
            (InvOn, Choice, "View", "Off".into(), strings(&["Off", "On"])),
            (InvPage, Choice, "Page", "DQ0".into(), strings(&["DQ0", "Mechanical"])),
            (InvA, Choice, "Phase A", "CH1".into(), u.wave_names()),
            (InvB, Choice, "Phase B", "CH2".into(), u.wave_names()),
            (InvC, Choice, "Phase C", "CH3".into(), u.wave_names()),
            (InvQty, Choice, "Signal", "Voltage".into(), strings(&["Voltage", "Current"])),
            (InvAng, Choice, "Angle", "Analog".into(), strings(&["Off", "Phase", "Analog", "Quad", "Hall"])),
            (InvAn, Choice, "Analog", "CH4".into(), u.wave_names()),
            (InvPoles, Value, "Poles", "3".into(), vec![]),
            (InvDeg, Value, "Deg/V", "2.500 °/V".into(), vec![]),
            (InvOff, Value, "Offset", "-5.000 °".into(), vec![]),
            (InvElec, Choice, "Angle is", "Elec".into(), strings(&["Mech", "Elec"])),
            (InvQa, Value, "A lead", "D2".into(), vec![]),
            (InvQb, Value, "B", "D3".into(), vec![]),
            (InvQi, Value, "Index", "Off".into(), vec![]),
            (InvCpr, Value, "Counts", "1234".into(), vec![]),
            (InvSwap, Choice, "Swap", "On".into(), strings(&["Off", "On"])),
            (InvHa, Value, "Hall A", "D4".into(), vec![]),
            (InvHb, Value, "Hall B", "D5".into(), vec![]),
            (InvHc, Value, "Hall C", "D6".into(), vec![]),
            (InvTq, Choice, "Torque", "Off".into(), u.wave_names_opt()),
            (InvScale, Value, "Scale", "12.0".into(), vec![]),
            (InvEv, Choice, "Bus V", "CH1".into(), u.wave_names_opt()),
            (InvEi, Choice, "Bus I", "CH2".into(), u.wave_names_opt()),
        ],
    );
}

#[test]
fn inverter_visibility_and_order_cover_both_pages_and_every_angle_mode() {
    use Item::*;
    let mut u = demo();
    for page in [Page::Dq0, Page::Mech] {
        u.m.inv_page = page;
        for (angle, dq, mech) in [
            (InvAngle::Off, vec![InvA, InvB, InvC, InvQty, InvAng, InvPoles], vec![InvAng, InvA, InvB, InvC, InvPoles]),
            (
                InvAngle::Phase,
                vec![InvA, InvB, InvC, InvQty, InvAng, InvPoles],
                vec![InvAng, InvA, InvB, InvC, InvPoles],
            ),
            (
                InvAngle::Analog,
                vec![InvA, InvB, InvC, InvQty, InvAng, InvAn, InvDeg, InvOff, InvElec],
                vec![InvAng, InvAn, InvDeg, InvOff, InvElec],
            ),
            (
                InvAngle::Quad,
                vec![InvA, InvB, InvC, InvQty, InvAng, InvQa, InvQb, InvQi, InvCpr, InvSwap],
                vec![InvAng, InvQa, InvQb, InvQi, InvCpr, InvSwap],
            ),
            (
                InvAngle::Hall,
                vec![InvA, InvB, InvC, InvQty, InvAng, InvHa, InvHb, InvHc],
                vec![InvAng, InvHa, InvHb, InvHc],
            ),
        ] {
            u.m.inv_ang = angle;
            for electrical in [false, true] {
                u.m.inv_elec = electrical;
                let mut expected = vec![InvOn, InvPage];
                expected.extend(if page == Page::Dq0 { dq.clone() } else { mech.clone() });
                if angle == InvAngle::Analog && electrical {
                    if page == Page::Dq0 {
                        expected.insert(8, InvPoles);
                    } else {
                        expected.push(InvPoles);
                    }
                }
                if page == Page::Mech {
                    expected.extend([InvTq, InvScale, InvEv, InvEi]);
                }
                assert_eq!(u.menu_items(MenuId::Inverter), expected, "{page:?} {angle:?} {electrical}");
                let options = if page == Page::Dq0 {
                    vec!["Off", "Phase", "Analog", "Quad", "Hall"]
                } else {
                    vec!["Phase", "Analog", "Quad", "Hall"]
                };
                assert_eq!(u.options(InvAng), options);
                let name = if page == Page::Mech && angle == InvAngle::Off {
                    "Phase"
                } else {
                    ["Off", "Phase", "Analog", "Quad", "Hall"][angle as usize]
                };
                assert_eq!(u.value(InvAng), name);
                assert_eq!(active_choice::index(&u, InvAng), options.iter().position(|o| *o == name));
            }
        }
    }
}

#[test]
fn inverter_values_keep_decorated_choices_and_logic_line_clamping() {
    use Item::*;
    let mut u = demo();
    for on in [false, true] {
        u.m.view = if on { AView::Inverter } else { AView::Scope };
        u.m.inv_volt = on;
        u.m.inv_elec = on;
        u.m.inv_swap = on;
        assert_eq!(u.value(InvOn), if on { "On" } else { "Off" });
        assert_eq!(u.value(InvQty), if on { "Voltage" } else { "Current" });
        assert_eq!(u.value(InvElec), if on { "Elec" } else { "Mech" });
        assert_eq!(u.value(InvSwap), if on { "On" } else { "Off" });
        for item in [InvOn, InvElec, InvSwap] {
            assert_eq!(active_choice::index(&u, item), Some(usize::from(on)));
        }
        assert_eq!(active_choice::index(&u, InvQty), Some(usize::from(!on)));
    }
    u.m.inv_poles = 0;
    assert_eq!(u.value(InvPoles), "1");
    for n in [0, 15, 16, 255] {
        (u.m.inv_qa, u.m.inv_qb, u.m.inv_ha, u.m.inv_hb, u.m.inv_hc) = (n, n, n, n, n);
        u.m.inv_qi = Some(n);
        for item in [InvQa, InvQb, InvQi, InvHa, InvHb, InvHc] {
            assert_eq!(u.value(item), format!("D{}", n.min(15)));
            assert_eq!(active_choice::index(&u, item), None);
        }
    }
    for (value, expected) in [(f64::NAN, "----"), (123.0, "123"), (12.0, "12.0"), (1.0, "1.000")] {
        u.m.inv_scale = value;
        assert_eq!(u.value(InvScale), expected);
    }
    for wave in [None, Some(WaveId::Ch(0)), Some(WaveId::Ref(0))] {
        (u.m.inv_tq, u.m.inv_ev, u.m.inv_ei) = (wave, wave, wave);
        let expected = wave.map(|w| u.wave_name(w)).unwrap_or_else(|| "Off".into());
        for item in [InvTq, InvEv, InvEi] {
            assert_eq!(u.value(item), expected);
            assert_eq!(active_choice::index(&u, item), u.wave_names_opt().iter().position(|s| *s == expected));
        }
    }
}

#[test]
fn inverter_source_prompt_keeps_its_original_page_and_angle_rules() {
    let mut u = demo();
    for channel in &mut u.m.ch {
        channel.on = false;
    }
    (u.m.inv_a, u.m.inv_b, u.m.inv_c, u.m.inv_an) = (WaveId::Ch(0), WaveId::Ch(0), WaveId::Ch(1), WaveId::Ch(2));
    (u.m.inv_tq, u.m.inv_ev, u.m.inv_ei) = (Some(WaveId::Ch(3)), Some(WaveId::Ch(3)), None);
    for page in [Page::Dq0, Page::Mech] {
        u.m.inv_page = page;
        for angle in [InvAngle::Off, InvAngle::Phase, InvAngle::Analog, InvAngle::Quad, InvAngle::Hall] {
            u.m.inv_ang = angle;
            let expected = match (page, angle) {
                (Page::Dq0, InvAngle::Analog) => vec![0, 1, 2],
                (Page::Dq0, _) => vec![0, 1],
                (Page::Mech, InvAngle::Phase) => vec![0, 1, 3],
                (Page::Mech, InvAngle::Analog) => vec![2, 3],
                (Page::Mech, _) => vec![3],
            };
            assert_eq!(u.off_sources(MenuId::Inverter), expected);
            assert_eq!(u.items(MenuId::Inverter).last(), Some(&Item::SrcOn));
        }
    }
}
