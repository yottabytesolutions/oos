// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::StoreSlot => Kind::Value,
        Item::StoreSaveSetup
        | Item::StoreRecall
        | Item::StoreCsv
        | Item::StorePng
        | Item::StoreFfSave
        | Item::StoreFfLoad => Kind::Action,
        Item::StoreBin => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Storage) => "Storage",
        Item::StoreSlot => "Setup slot",
        Item::StoreDest => "Save to",
        Item::StoreSaveSetup => "Save setup",
        Item::StoreRecall => "Recall setup",
        Item::StoreCsv => "Save waveform (CSV)",
        Item::StorePng => "Save screenshot (PNG)",
        Item::StoreFfSave => "Save fast-frame",
        Item::StoreFfLoad => "Load fast-frame",
        Item::StoreBin => "Save waveform (binary)",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    Some(match it {
        Item::StoreDest => ui.store_dest.name().into(),
        Item::StoreSlot => format!("{}  (setup{}.txt)", ui.store_slot, ui.store_slot),
        _ => return None,
    })
}

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

fn options(_ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::StoreDest => return Some(crate::store::StoreDest::ALL.iter().map(|d| d.name().to_string()).collect()),
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(_ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Storage => vec![
            Item::StoreDest,
            Item::StoreSlot,
            Item::StoreSaveSetup,
            Item::StoreRecall,
            Item::StoreCsv,
            Item::StoreBin,
            Item::StoreFfSave,
            Item::StoreFfLoad,
            Item::StorePng,
        ],
        _ => Vec::new(),
    }
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
