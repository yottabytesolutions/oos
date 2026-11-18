// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust, activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::title(Open(MenuId::Storage), "Storage"),
        Descriptor::choice(
            StoreDest,
            "Save to",
            |u| u.store_dest.name().into(),
            |_| crate::store::StoreDest::ALL.iter().map(|d| d.name().into()).collect(),
        ),
        Descriptor::value(StoreSlot, "Setup slot", |u| format!("{}  (setup{}.txt)", u.store_slot, u.store_slot)),
        Descriptor::action(StoreSaveSetup, "Save setup"),
        Descriptor::action(StoreRecall, "Recall setup"),
        Descriptor::action(StoreCsv, "Save waveform (CSV)"),
        Descriptor::action(StoreBin, "Save waveform (binary)"),
        Descriptor::action(StoreFfSave, "Save fast-frame"),
        Descriptor::action(StoreFfLoad, "Load fast-frame"),
        Descriptor::action(StorePng, "Save screenshot (PNG)"),
    ]
};

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::StoreSlot => ui.store_slot = (ui.store_slot as i32 + d).clamp(1, 9) as u32,
        Item::StoreDest => {
            let i = crate::store::StoreDest::ALL.iter().position(|&x| x == ui.store_dest).unwrap_or(0);
            ui.store_dest = crate::store::StoreDest::ALL[cyc(i, crate::store::StoreDest::ALL.len(), d)];
        }
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::StoreBin => match &ui.last[0] {
            Some(rec) => {
                let p = ui.new_file(&ui.save_dir(), "wave", "bin");
                let r = crate::store::write(&p, &crate::store::wave_bin(rec, &ui.m));
                ui.toast_saved(&p, r);
            }
            None => ui.toast("No waveform to save"),
        },
        Item::StoreSaveSetup => crate::ui::lifecycle::storage::save(ui),
        Item::StoreRecall => ui.recall_setup(),
        Item::StoreCsv => match &ui.last[0] {
            Some(rec) => {
                let p = ui.new_file(&ui.save_dir(), "wave", "csv");
                let mut r = crate::store::write(&p, crate::store::wave_csv(rec, &ui.m).as_bytes());
                // the LA lines go to their own file: another sample rate, and only the changes
                if let (Ok(()), Some(la), true) = (&r, &rec.la, ui.m.la_on()) {
                    let q = ui.new_file(&ui.save_dir(), "la", "csv");
                    r = crate::store::write(&q, crate::store::la_csv(la, &ui.m.la.lines()).as_bytes());
                }
                ui.toast_saved(&p, r);
            }
            None => ui.toast("No waveform to save"),
        },
        Item::StorePng => {
            // without the dialog: the screen is saved after the next frame (main loop / script runner)
            let p = ui.new_file(&ui.save_dir(), "shot", "png");
            ui.close_menu();
            ui.shot_request = Some(p.to_string_lossy().into_owned());
        }
        Item::StoreFfSave => ui.ff_save(),
        Item::StoreFfLoad => ui.ff_load(),
        _ => {}
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_area_save_uses_worker_admission_and_completion() {
        let dir = std::env::temp_dir().join(format!("oos-storage-menu-{}", std::process::id()));
        let mut ui = Ui::for_tests(crate::Source::Demo { t: 0.0 });
        ui.store_dir = dir.clone();
        let expected = format!("{}{}", crate::store::setup_text(&ui.m), ui.ref_setup_text());
        assert!(activate(&mut ui, Item::StoreSaveSetup));
        assert!(crate::ui::lifecycle::storage::get(&mut ui, None).unwrap().busy());
        crate::ui::lifecycle::storage::poll(&mut ui, true).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("setup1.txt")).unwrap(), expected);
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[cfg(test)]
mod read_tests;
