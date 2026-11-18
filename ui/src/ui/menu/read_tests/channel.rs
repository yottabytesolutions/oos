// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn every_channel_row_keeps_its_read_contract_and_order() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    for c in 0..n_ch() {
        for on in [false, true] {
            u.m.ch[c].on = on;
            u.m.ch[c].invert = on;
            u.m.fine_v = on;
            u.m.v_expand_gnd = on;
            u.m.ch[c].offset = -0.25;
            u.m.ch[c].bias = 0.125;
            u.m.ch[c].skew = 1.2e-9;
            let toggle = if on { "On" } else { "Off" };
            let rows = [
                (ChOn(c), Choice, "Display", toggle.into(), strings(&["Off", "On"])),
                (ChScale(c), Value, "Scale", si_short(u.m.ch[c].volts_per_div() as f64, u.m.unit(c)), vec![]),
                (ChOffset(c), Value, "Offset", si(u.m.ch[c].offset as f64, u.m.unit(c)), vec![]),
                (
                    ChCoupling(c),
                    Choice,
                    "Coupling",
                    u.m.ch[c].coupling.name().into(),
                    board().couplings().iter().map(|v| v.name().into()).collect(),
                ),
                (ChBw(c), Choice, "Bandwidth", BWS[u.m.ch[c].bw].0.into(), BWS.iter().map(|v| v.0.into()).collect()),
                (ChProbe(c), Choice, "Probe", format!("{}x", u.m.ch[c].probe), strings(&["1x", "10x", "100x"])),
                (ChInvert(c), Choice, "Invert", toggle.into(), strings(&["Off", "On"])),
                (ChFine, Choice, "Fine scale", toggle.into(), strings(&["Off", "On"])),
                (ChLabel(c), Action, "Name", "".into(), vec![]),
                (Open(MenuId::ChMore(c)), Action, "More", "".into(), vec![]),
                (ChUnit(c), Choice, "Unit", u.m.unit(c).into(), strings(&["V", "A", "W", "U"])),
                (ChBias(c), Value, "Zero bias", si(u.m.ch[c].bias as f64, u.m.unit(c)), vec![]),
                (ChBiasZero(c), Action, "Zero this input", "".into(), vec![]),
                (ChSkew(c), Value, "Deskew", si(u.m.ch[c].skew as f64, "s"), vec![]),
                (AutoDeskew, Action, "Auto deskew", "".into(), vec![]),
                (
                    ChExpand,
                    Choice,
                    "Expand around",
                    if on { "Ground" } else { "Center" }.into(),
                    strings(&["Center", "Ground"]),
                ),
            ];
            super::super::small_tests::read_rows(&u, &rows);
            assert_eq!(u.menu_items(MenuId::Channel(c)), rows[..10].iter().map(|r| r.0).collect::<Vec<_>>());
            assert_eq!(u.menu_items(MenuId::ChMore(c)), rows[10..].iter().map(|r| r.0).collect::<Vec<_>>());
            for item in [ChOn(c), ChInvert(c), ChFine, ChExpand] {
                assert_eq!(active_choice::index(&u, item), Some(usize::from(on)));
            }
        }
    }
}

#[test]
fn channel_choices_keep_values_and_committed_identity() {
    use Item::*;
    let mut u = demo();
    for c in 0..n_ch() {
        for coupling in board().couplings() {
            u.m.ch[c].coupling = *coupling;
            assert_eq!(u.value(ChCoupling(c)), coupling.name());
        }
        for (index, (label, _)) in BWS.iter().enumerate() {
            u.m.ch[c].bw = index;
            assert_eq!(u.value(ChBw(c)), *label);
        }
        for probe in [1.0, 10.0, 100.0, 2.5] {
            u.m.ch[c].probe = probe;
            assert_eq!(u.value(ChProbe(c)), format!("{probe}x"));
            if probe != 2.5 {
                assert_eq!(u.options(ChProbe(c))[active_choice::index(&u, ChProbe(c)).unwrap()], u.value(ChProbe(c)));
            }
        }
        for unit in AmpUnit::ALL {
            u.m.ch[c].unit = unit;
            assert_eq!(u.value(ChUnit(c)), u.m.unit(c));
            assert_eq!(u.options(ChUnit(c))[active_choice::index(&u, ChUnit(c)).unwrap()], u.value(ChUnit(c)));
            assert_eq!(u.value(ChScale(c)), si_short(u.m.ch[c].volts_per_div() as f64, u.m.unit(c)));
        }
    }
}

#[test]
fn channel_descriptors_have_one_owner_per_item() {
    let rows = super::super::channel::AREA.descriptors;
    let items: std::collections::HashSet<_> = rows.iter().map(|row| row.item).collect();
    assert_eq!(items.len(), rows.len());
    for item in [Item::ChFine, Item::ChExpand] {
        assert_eq!(rows.iter().filter(|row| row.item == item).count(), 1);
    }
}
