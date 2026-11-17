// SPDX-License-Identifier: GPL-2.0-or-later

//! SCPI edits use the same row order and selection limit as the Measure readout.

use super::*;

pub(super) fn query(scpi: &mut Scpi, ui: &mut Ui, cv: &Canvas, command: &str) -> String {
    String::from_utf8(scpi.exec_line(ui, cv, command)).expect("SCPI text replies are UTF-8")
}

pub(super) fn catalog_row(ui: &mut Ui) {
    let (_, def) = crate::measures::all()
        .find(|(_, def)| def.kind == crate::measures::Kind::Module)
        .expect("the registry includes module measurements");
    ui.m.cat.push((def.id, WaveId::Ch(0), WaveId::Ch(1), WaveId::Ch(0), WaveId::Ch(1)));
}

#[test]
fn meas_clear_item_uses_one_based_screen_order() {
    let (mut ui, cv, mut scpi) = meas_setup();
    query(&mut scpi, &mut ui, &cv, ":MEAS:ITEM RRPH,CHAN1,CHAN2;:MEAS:ITEM VPP;:MEAS:ITEM FREQ,CHAN2");
    catalog_row(&mut ui);
    let catalog = ui.meas_list().0.pop().expect("one catalog row was selected");
    assert_eq!(ui.meas_list().0, ["Pk-pk 1", "Freq 2", "Phase rise-rise 1-2 1", &catalog]);
    assert_eq!(query(&mut scpi, &mut ui, &cv, ":MEASure:CLEar ITEM2"), "");
    assert_eq!(ui.meas_list().0, ["Pk-pk 1", "Phase rise-rise 1-2 1", &catalog]);
    assert_eq!(query(&mut scpi, &mut ui, &cv, ":meas:cle item2"), "");
    assert_eq!(ui.meas_list().0, ["Pk-pk 1", &catalog]);
    assert_eq!(query(&mut scpi, &mut ui, &cv, ":MEAS:CLE ITEM2"), "");
    assert_eq!(ui.meas_list().0, ["Pk-pk 1"]);
    assert_eq!(query(&mut scpi, &mut ui, &cv, ":SYST:ERR?"), "0,\"No error\"");
    assert_eq!(query(&mut scpi, &mut ui, &cv, ":MEAS:CLE ITEM1"), "");
    assert!(ui.meas_list().0.is_empty());
}

#[test]
fn meas_clear_invalid_item_preserves_rows_and_statistics() {
    let (mut ui, cv, mut scpi) = meas_setup();
    query(&mut scpi, &mut ui, &cv, ":MEAS:ITEM VPP;:MEAS:ITEM FREQ,CHAN2");
    q_steps(&mut ui, 4);
    let rows = ui.meas_list().0;
    let stats = ui.measurement_statistics_snapshot_for_test();
    assert!(stats.iter().any(|s| s.0 > 0));
    for arg in ["ITEM0", "ITEM3", "ITEM1025", "ITEM", "ITEM-1", "ITEM1.0", "ITEM1,ITEM2", "BOGUS"] {
        assert_eq!(query(&mut scpi, &mut ui, &cv, &format!(":MEAS:CLE {arg}")), "");
        assert!(query(&mut scpi, &mut ui, &cv, ":SYST:ERR?").starts_with("-224,"), "{arg}");
        assert_eq!(ui.meas_list().0, rows, "{arg}");
        assert_eq!(ui.measurement_statistics_snapshot_for_test(), stats, "{arg}");
    }
    query(&mut scpi, &mut ui, &cv, ":MEAS:CLE ITEM1");
    assert!(ui.measurement_statistics_snapshot_for_test().is_empty());
    assert_eq!(ui.meas_list().0, ["Freq 2"]);
}

#[test]
fn meas_clear_empty_list_rejects_item_but_accepts_clear_all() {
    let (mut ui, cv, mut scpi) = meas_setup();
    for arg in ["ITEM1", "ITEM0", "ITEM2"] {
        assert_eq!(query(&mut scpi, &mut ui, &cv, &format!(":MEAS:CLE {arg}")), "");
        assert!(query(&mut scpi, &mut ui, &cv, ":SYST:ERR?").starts_with("-224,"), "{arg}");
        assert!(ui.meas_list().0.is_empty());
    }
    for clear in [":MEASure:CLEar ALL", ":MEAS:CLE"] {
        query(&mut scpi, &mut ui, &cv, ":MEAS:ITEM VPP;:MEAS:ITEM RRPH,CHAN1,CHAN2");
        catalog_row(&mut ui);
        assert_eq!(query(&mut scpi, &mut ui, &cv, clear), "");
        assert!(ui.meas_list().0.is_empty());
        assert_eq!(query(&mut scpi, &mut ui, &cv, ":SYST:ERR?"), "0,\"No error\"");
        query(&mut scpi, &mut ui, &cv, clear);
        assert_eq!(query(&mut scpi, &mut ui, &cv, ":SYST:ERR?"), "0,\"No error\"");
    }
}

// Break it catches: a reset that only takes effect at the next step, so a query later on the same program line
// still counts the old history. SCPI commands are sequential (IEEE 488.2).
#[test]
fn statistics_reset_is_seen_by_the_next_query_on_the_same_line() {
    let (mut ui, cv, mut scpi) = meas_setup();
    query(&mut scpi, &mut ui, &cv, ":MEAS:ITEM VPP,CHAN1");
    q_steps(&mut ui, 30);
    let count: f64 = query(&mut scpi, &mut ui, &cv, ":MEAS:STAT:ITEM? CNT,VPP,CHAN1").parse().expect("a count");
    assert!(count >= 1.0, "{count}");
    assert_eq!(query(&mut scpi, &mut ui, &cv, ":MEAS:STAT:RES;:MEAS:STAT:ITEM? CNT,VPP,CHAN1"), "0.000000E+00");
}
