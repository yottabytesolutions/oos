// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::small_tests::{demo, read_rows, strings};
use super::*;

#[test]
fn every_storage_row_preserves_labels_values_options_and_order() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    read_rows(
        &u,
        &[
            (Open(MenuId::Storage), Action, "Storage", "".into(), vec![]),
            (StoreSaveSetup, Action, "Save setup", "".into(), vec![]),
            (StoreRecall, Action, "Recall setup", "".into(), vec![]),
            (StoreCsv, Action, "Save waveform (CSV)", "".into(), vec![]),
            (StoreBin, Action, "Save waveform (binary)", "".into(), vec![]),
            (StorePng, Action, "Save screenshot (PNG)", "".into(), vec![]),
            (StoreFfSave, Action, "Save fast-frame", "".into(), vec![]),
            (StoreFfLoad, Action, "Load fast-frame", "".into(), vec![]),
        ],
    );
    for slot in 1..=9 {
        u.store_slot = slot;
        read_rows(&u, &[(StoreSlot, Value, "Setup slot", format!("{slot}  (setup{slot}.txt)"), vec![])]);
        assert_eq!(active_choice::index(&u, StoreSlot), Some(slot as usize - 1));
    }
    for (index, dest) in crate::store::StoreDest::ALL.into_iter().enumerate() {
        u.store_dest = dest;
        read_rows(
            &u,
            &[(
                StoreDest,
                Choice,
                "Save to",
                ["Auto", "USB", "Local"][index].into(),
                strings(&["Auto", "USB", "Local"]),
            )],
        );
        assert_eq!(active_choice::index(&u, StoreDest), Some(index));
    }
    assert_eq!(
        u.menu_items(MenuId::Storage),
        [StoreDest, StoreSlot, StoreSaveSetup, StoreRecall, StoreCsv, StoreBin, StoreFfSave, StoreFfLoad, StorePng]
    );
}

#[test]
fn contexts_stay_out_of_visible_rows() {
    super::super::small_tests::contexts(MenuId::Storage, &[Item::Open(MenuId::Storage)]);
}

#[test]
fn navigation_and_visibility_at_every_text_size() {
    super::super::small_tests::navigation(MenuId::Storage, Item::StoreDest, |u, focused| {
        u.store_dest = crate::store::StoreDest::ALL[focused]
    });
}

#[test]
fn capabilities_in_isolated_processes() {
    super::super::small_tests::capabilities(
        MenuId::Storage,
        "ui::menu::storage::read_tests::capabilities_in_isolated_processes",
        || {},
    );
}
