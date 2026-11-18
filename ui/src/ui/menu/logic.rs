// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

use crate::ui::la::{GROUPS, HEIGHTS, NAMES, PAT_BITS, PRESETS, USER};

pub(super) const AREA: Area = Area {
    descriptors: ROWS,
    adjust: Ui::la_adjust,
    rows: |_, menu| match menu {
        MenuId::La => &ROWS[..9],
        MenuId::LaLines => &ROWS[11..27],
        MenuId::LaPat => &ROWS[27..],
        _ => &[],
    },
    ..Area::EMPTY
};

macro_rules! logic_rows {
    ($($n:literal),+) => {{
        use Item::*;
        &[
            Descriptor::action(Open(MenuId::La), "Logic analyser").when(|_| false),
            Descriptor::choice(LaOn, "LA", |u| onoff(u.m.la.on), toggles),
            Descriptor::choice(
                LaShow,
                "Lines",
                |u| GROUPS.iter().take(3).find(|g| g.1 == u.m.la.show).map_or(GROUPS[3].0, |g| g.0).into(),
                |_| GROUPS.iter().map(|g| g.0.to_string()).collect(),
            ),
            Descriptor::action(Open(MenuId::LaLines), "Line select"),
            Descriptor::choice(
                LaHeight,
                "Lane height",
                |u| HEIGHTS[u.m.la.height.min(HEIGHTS.len() - 1)].0.into(),
                |_| HEIGHTS.iter().map(|h| h.0.to_string()).collect(),
            ),
            threshold::<0>(),
            user::<0>(),
            threshold::<1>(),
            user::<1>(),
            Descriptor::new(
                Open(MenuId::LaPat),
                Kind::Action,
                "D0-D15",
                |u| {
                    u.m.pat_la
                        .iter()
                        .rev()
                        .map(|b| match b {
                            Bit::H => '1',
                            Bit::L => '0',
                            _ => 'x',
                        })
                        .collect()
                },
                |_| Vec::new(),
                |_| false,
            ),
            Descriptor::choice(
                DecIn,
                "Input",
                |u| if u.m.dec_la_in { "LA" } else { "Analog" }.into(),
                |_| ["Analog", "LA"].into_iter().map(String::from).collect(),
            )
            .when(|u| u.m.la_on() && u.m.dec_proto != Proto::CPHY),
            $(line::<$n>()),+,
            $(pattern::<$n>()),+,
        ]
    }};
}

const ROWS: &[Descriptor] = logic_rows!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);

const fn threshold<const P: usize>() -> Descriptor {
    Descriptor::choice(
        Item::LaThr(P),
        ["Thr D0-7", "Thr D8-15"][P],
        |u| PRESETS[u.m.la.preset[P].min(USER)].0.into(),
        |_| PRESETS.iter().map(|p| p.0.to_string()).collect(),
    )
}

const fn user<const P: usize>() -> Descriptor {
    Descriptor::value(Item::LaUser(P), ["User D0-7", "User D8-15"][P], |u| format!("{:.2} V", u.m.la.user[P]))
        .when(|u| u.m.la.preset[P] == USER)
}

const fn line<const N: usize>() -> Descriptor {
    Descriptor::choice(Item::LaLine(N), NAMES[N], |u| onoff(u.m.la.show >> N & 1 == 1), toggles)
}

const fn pattern<const N: usize>() -> Descriptor {
    Descriptor::choice(
        Item::LaPat(N),
        NAMES[N],
        |u| ["H", "L", "X"][PAT_BITS.iter().position(|&b| b == u.m.pat_la[N]).unwrap_or(2)].into(),
        |_| ["H", "L", "X"].into_iter().map(String::from).collect(),
    )
}
