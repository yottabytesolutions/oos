// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) fn ordered_rows(ui: &Ui, _: MenuId) -> Vec<&'static Descriptor> {
    use Item::*;
    let body: &[Item] = match ui.m.trig_type {
        TT::Edge => &[TrigSrc, TrigSlope, TrigLevel, Trig50, TrigCoup],
        TT::Bus => &[DecProto, TrigBusWhat, TrigBusVal],
        TT::Pulse => &[TrigSrc, TrigPol, TrigWhen, TrigT1, TrigT2, TrigLevel],
        TT::Slope => &[TrigSrc, TrigSlope, TrigWhen, TrigT1, TrigT2, TrigLevel, TrigLevel2],
        TT::Video => &[TrigSrc, VidStd, VidSync, VidLine, TrigLevel],
        TT::Pattern => &[Pat(0), Pat(1), Pat(2), Pat(3), TrigSrc, PatLevel, Open(MenuId::LaPat)],
        TT::Duration => {
            &[Pat(0), Pat(1), Pat(2), Pat(3), TrigWhen, TrigT1, TrigT2, TrigSrc, PatLevel, Open(MenuId::LaPat)]
        }
        TT::Timeout => &[TrigSrc, TrigSlope, TrigT1, TrigLevel],
        TT::Runt => &[TrigSrc, TrigPol, TrigWhen, TrigT1, TrigT2, TrigLevel, TrigLevel2],
        TT::Window => &[TrigSrc, WinModeI, TrigWhen, TrigT1, TrigT2, TrigLevel, TrigLevel2],
        TT::Delay => &[TrigSrc, TrigSlope, TrigSrc2, TrigSlope2, TrigWhen, TrigT1, TrigT2, TrigLevel, TrigLevel2],
        TT::SetupHold => &[TrigSrc, TrigSrc2, TrigSlope2, Setup, Hold, TrigLevel, TrigLevel2],
        TT::NthEdge => &[TrigSrc, TrigSlope, NthIdle, NthN, TrigLevel],
    };
    let serial = if ui.m.trig_type == TT::Bus { ui.serial_items() } else { Vec::new() };
    resolve_rows([TrigType].iter().chain(body).copied().chain(serial).chain([
        TrigNoise,
        TrigAutoNoise,
        TrigMode,
        SearchOn,
        SearchTable,
        SearchSave,
        Open(MenuId::Zone),
        TrigHoldoff,
    ]))
}

fn resolve_rows(items: impl Iterator<Item = Item>) -> Vec<&'static Descriptor> {
    let rows: std::collections::HashMap<_, _> = super::super::decode::AREA
        .descriptors
        .iter()
        .chain(super::super::serial::AREA.descriptors)
        .chain(ROWS)
        .map(|row| (row.item, row))
        .collect();
    items.filter_map(|item| rows.get(&item).copied()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unregistered_rows_are_skipped_without_losing_the_order() {
        let items = [Item::TrigType, Item::ChOn(0), Item::TrigMode];
        let rows = resolve_rows(items.into_iter());
        assert_eq!(rows.iter().map(|row| row.item).collect::<Vec<_>>(), [Item::TrigType, Item::TrigMode]);
    }
}
