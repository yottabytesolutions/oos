// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::ui::la::{HEIGHTS, PRESETS, USER};

#[test]
fn every_logic_page_row_preserves_reads_and_threshold_visibility() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    check(&u, Open(MenuId::La), Action, "Logic analyser", "", vec![]);
    check(&u, Open(MenuId::LaLines), Action, "Line select", "", vec![]);
    check(&u, DecIn, Choice, "Input", "Analog", strings(&["Analog", "LA"]));
    u.m.dec_la_in = true;
    assert_eq!(u.value(DecIn), "LA");
    for on in [false, true] {
        u.m.la.on = on;
        check(&u, LaOn, Choice, "LA", if on { "On" } else { "Off" }, strings(&["Off", "On"]));
    }
    for (mask, name) in [(0xff, "D0-D7"), (0xff00, "D8-D15"), (0xffff, "All"), (0x1234, "Custom")] {
        u.m.la.show = mask;
        check(&u, LaShow, Choice, "Lines", name, strings(&["D0-D7", "D8-D15", "All", "Custom"]));
    }
    for (height, name) in ["Small", "Medium", "Large"].into_iter().enumerate() {
        u.m.la.height = height;
        check(&u, LaHeight, Choice, "Lane height", name, HEIGHTS.iter().map(|h| h.0.into()).collect());
    }
    for a in 0..=USER {
        for b in 0..=USER {
            u.m.la.preset = [a, b];
            let mut want = vec![LaOn, LaShow, Open(MenuId::LaLines), LaHeight];
            for p in 0..2 {
                let preset = u.m.la.preset[p];
                let thr_label = ["Thr D0-7", "Thr D8-15"][p];
                let user_label = ["User D0-7", "User D8-15"][p];
                check(&u, LaThr(p), Choice, thr_label, PRESETS[preset].0, PRESETS.iter().map(|p| p.0.into()).collect());
                u.m.la.user[p] = -0.75 + p as f32;
                check(&u, LaUser(p), Value, user_label, if p == 0 { "-0.75 V" } else { "0.25 V" }, vec![]);
                want.push(LaThr(p));
                if preset == USER {
                    want.push(LaUser(p));
                }
            }
            assert_eq!(u.menu_items(MenuId::La), want);
        }
    }
}

#[test]
fn every_logic_line_and_pattern_row_keeps_its_index() {
    use Item::*;
    let mut u = demo();
    assert_eq!(u.menu_items(MenuId::LaLines), (0..16).map(LaLine).collect::<Vec<_>>());
    assert_eq!(u.menu_items(MenuId::LaPat), (0..16).map(LaPat).collect::<Vec<_>>());
    for n in 0..16 {
        for on in [false, true] {
            u.m.la.show = if on { 1 << n } else { 0 };
            check(
                &u,
                LaLine(n),
                Kind::Choice,
                &format!("D{n}"),
                if on { "On" } else { "Off" },
                strings(&["Off", "On"]),
            );
        }
        for (bit, name) in [(Bit::H, "H"), (Bit::L, "L"), (Bit::X, "X")] {
            u.m.pat_la[n] = bit;
            check(&u, LaPat(n), Kind::Choice, &format!("D{n}"), name, strings(&["H", "L", "X"]));
        }
    }
    u.m.pat_la = [Bit::X; 16];
    u.m.pat_la[0] = Bit::H;
    u.m.pat_la[15] = Bit::L;
    check(&u, Open(MenuId::LaPat), Kind::Action, "D0-D15", "0xxxxxxxxxxxxxx1", vec![]);
}
