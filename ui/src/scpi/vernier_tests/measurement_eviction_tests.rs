// SPDX-License-Identifier: GPL-2.0-or-later

//! SCPI additions evict the first screen row at capacity; menu additions still refuse.

use super::measurement_list_tests::{catalog_row, query};
use super::*;

// Fill with distinct valid pair selections; missing reference data is allowed in a saved selection.
fn fill_pairs(ui: &mut Ui) {
    let room = ui.meas_list().1;
    for (_, def) in crate::measures::all() {
        let crate::measures::Kind::Pair(kind) = def.kind else { continue };
        for a in 0..NREF {
            for b in 0..NREF {
                if a == b {
                    continue;
                }
                if ui.m.meas.len() + ui.m.meas2.len() + ui.m.cat.len() == room {
                    return;
                }
                ui.m.meas2.push((kind, WaveId::Ref(a), WaveId::Ref(b)));
            }
        }
    }
    panic!("the pair catalog must be large enough to fill the selection");
}

#[test]
fn meas_evict_first_row_for_standard_and_pair_additions() {
    for standard_first in [true, false] {
        let (mut ui, cv, mut scpi) = meas_setup();
        if standard_first {
            query(&mut scpi, &mut ui, &cv, ":MEAS:ITEM VPP");
        }
        fill_pairs(&mut ui);
        let (mut expected, room) = ui.meas_list();
        assert_eq!(expected.len(), room);
        expected.remove(0);
        expected.insert(0, "Freq 2".into());
        assert_eq!(query(&mut scpi, &mut ui, &cv, ":MEAS:ITEM FREQ,CHAN2"), "");
        assert_eq!(ui.meas_list().0, expected);
        assert_eq!(query(&mut scpi, &mut ui, &cv, ":SYST:ERR?"), "0,\"No error\"");
        // A duplicate at capacity must neither evict nor reorder anything.
        query(&mut scpi, &mut ui, &cv, ":MEAS:ITEM FREQ,CHAN2");
        assert_eq!(ui.meas_list().0, expected);
        // Same-source pairs are rejected before making room.
        query(&mut scpi, &mut ui, &cv, ":MEAS:ITEM RRD,CHAN1,CHAN1");
        assert!(query(&mut scpi, &mut ui, &cv, ":SYST:ERR?").starts_with("-224,"));
        assert_eq!(ui.meas_list().0, expected);
        expected.remove(0);
        expected.push("Delay rise-rise 1-2 1".into());
        query(&mut scpi, &mut ui, &cv, ":MEAS:STAT:ITEM RRD,CHAN1,CHAN2");
        assert_eq!(ui.meas_list().0, expected);
        query(&mut scpi, &mut ui, &cv, ":MEAS:ITEM RRD,CHAN1,CHAN2");
        assert_eq!(ui.meas_list().0, expected);
        assert_eq!(query(&mut scpi, &mut ui, &cv, ":SYST:ERR?"), "0,\"No error\"");
    }
}

#[test]
fn meas_evict_first_row_from_full_pair_list() {
    let (mut ui, cv, mut scpi) = meas_setup();
    fill_pairs(&mut ui);
    let mut expected = ui.meas_list().0;
    expected.remove(0);
    expected.push("Delay rise-rise 1-2 1".into());
    query(&mut scpi, &mut ui, &cv, ":MEAS:ITEM RRD,CHAN1,CHAN2");
    assert_eq!(ui.meas_list().0, expected);
    assert_eq!(query(&mut scpi, &mut ui, &cv, ":SYST:ERR?"), "0,\"No error\"");
}

#[test]
fn meas_evict_first_row_from_full_catalog_list() {
    let (mut ui, cv, mut scpi) = meas_setup();
    catalog_row(&mut ui);
    let id = ui.m.cat[0].0;
    ui.m.cat = (0..ui.meas_list().1)
        .map(|i| {
            (
                id,
                WaveId::Ref(i % NREF),
                WaveId::Ref(i / NREF % NREF),
                WaveId::Ref(i / NREF.pow(2) % NREF),
                WaveId::Ref(i / NREF.pow(3) % NREF),
            )
        })
        .collect();
    let mut expected = ui.meas_list().0;
    expected.remove(0);
    expected.insert(0, "Pk-pk 1".into());
    query(&mut scpi, &mut ui, &cv, ":MEAS:ITEM VPP");
    assert_eq!(ui.meas_list().0, expected);
    assert_eq!(ui.m.cat.len(), ui.meas_list().1 - 1);
    assert_eq!(ui.m.cat[0].1, WaveId::Ref(1));
    assert_eq!(query(&mut scpi, &mut ui, &cv, ":SYST:ERR?"), "0,\"No error\"");
}
