// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn every_reference_row_keeps_its_reads_including_empty_and_named_slots() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.ref_src = 1;
    u.ref_files = vec!["capture.csv".into(), "trace.bin".into()];
    let rows = [
        (RefSrc, Choice, "Save from", "CH2", ch_names()),
        (RefSave, Action, "Save to next R", "", vec![]),
        (RefClear, Action, "Clear all", "", vec![]),
        (RefSel, Value, "Slot", "R1", vec![]),
        (RefVpd, Value, "Slot scale", "-", vec![]),
        (RefOffset, Value, "Slot offset", "-", vec![]),
        (RefColor, Value, "Slot color", "-", vec![]),
        (RefLabel, Action, "Slot name", "", vec![]),
        (RefLoad, Action, "Load file", "", vec![]),
        (RefExport, Action, "Export slot", "", vec![]),
        (RefImport, Action, "Import slot", "", strings(&["capture.csv", "trace.bin"])),
    ];
    for (it, kind, label, value, options) in rows {
        check(&u, it, kind, label, value, options);
    }
    check(&u, Open(MenuId::Ref), Action, "Reference", "", vec![]);
    for n in 0..NREF {
        check(&u, RefShow(n), Choice, &format!("R{}", n + 1), "empty", vec![]);
        u.refs[n] = Some(wave());
        u.ref_sel = n;
        assert_eq!(u.value(RefSel), format!("R{}", n + 1));
        assert_eq!(u.value(RefVpd), "2 V/div");
        assert_eq!(u.value(RefOffset), "-250 mV");
        assert_eq!(u.value(RefColor), REF_COLORS[2].0);
        assert_eq!(u.value(RefShow(n)), "CH2 on");
        let r = u.refs[n].as_mut().unwrap();
        r.label = "Saved".into();
        r.on = false;
        r.color = usize::MAX;
        assert_eq!(u.value(RefShow(n)), "Saved off");
        assert_eq!(u.value(RefColor), REF_COLORS.last().unwrap().0);
    }
}

#[test]
fn reference_visibility_changes_only_the_load_row() {
    use Item::*;
    let mut u = demo();
    for enabled in [false, true] {
        u.sys.features_off = if enabled { vec![] } else { vec!["load".into()] };
        let mut want = vec![RefSrc, RefSave, RefClear, RefSel, RefVpd, RefOffset, RefColor, RefLabel];
        if enabled {
            want.push(RefLoad);
        }
        want.extend([RefExport, RefImport]);
        want.extend((0..NREF).map(RefShow));
        assert_eq!(u.menu_items(MenuId::Ref), want);
    }
}
