// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn every_bus_row_preserves_its_reads_and_order() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.m.bus.label = "CONTROL".into();
    u.m.bus.pos = -1.5;
    u.m.bus.clk = WaveId::Ch(1);
    u.m.bus.data = WaveId::Ref(3);
    u.m.bus.clk_thr = 1.25;
    u.m.bus.data_thr = -0.5;
    let sources: Vec<_> = ch_names().into_iter().chain((1..=NREF).map(|n| format!("R{n}"))).collect();
    let expected = [
        (BusOn, Choice, "Bus", "Off", strings(&["Off", "On"])),
        (BusLabel, Action, "Label", "CONTROL", vec![]),
        (BusPos, Value, "Position", "-1.5 div", vec![]),
        (BusClk, Choice, "Clock", "CH2", sources.clone()),
        (BusClkThr, Value, "Clock threshold", "1.25 V", vec![]),
        (BusData, Choice, "Data", "R4", sources),
        (BusDataThr, Value, "Data threshold", "-500 mV", vec![]),
        (BusRw, Choice, "R/W bit", "Left out", strings(&["Left out", "Included"])),
    ];
    assert_eq!(u.menu_items(MenuId::Bus), expected.iter().map(|r| r.0).collect::<Vec<_>>());
    for (it, kind, label, value, options) in expected {
        check(&u, it, kind, label, value, options);
    }
    check(&u, Open(MenuId::Bus), Action, "Bus", "", vec![]);
    u.m.bus.on = true;
    u.m.bus.addr_rw = true;
    u.m.bus.label.clear();
    assert_eq!(u.value(BusOn), "On");
    assert_eq!(u.value(BusRw), "Included");
    assert_eq!(u.value(BusLabel), u.m.bus.shown_label());
}
