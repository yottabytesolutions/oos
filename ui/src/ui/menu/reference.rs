// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::RefColor | Item::RefSel | Item::RefVpd | Item::RefOffset => Kind::Value,
        Item::RefSave | Item::RefClear => Kind::Action,
        Item::RefLabel | Item::RefExport | Item::RefImport | Item::RefLoad => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Ref) => "Reference",
        Item::RefSrc => "Save from",
        Item::RefSave => "Save to next R",
        Item::RefShow(i) => ["R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "R9", "R10"][i.min(NREF - 1)],
        Item::RefClear => "Clear all",
        Item::RefColor => "Slot color",
        Item::RefLabel => "Slot name",
        Item::RefSel => "Slot",
        Item::RefVpd => "Slot scale",
        Item::RefOffset => "Slot offset",
        Item::RefExport => "Export slot",
        Item::RefImport => "Import slot",
        Item::RefLoad => "Load file",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    Some(match it {
        Item::RefColor => {
            ui.refs[ui.ref_sel].as_ref().map_or("-", |r| REF_COLORS[r.color.min(REF_COLORS.len() - 1)].0).into()
        }
        Item::RefSel => format!("R{}", ui.ref_sel + 1),
        Item::RefVpd => {
            ui.refs[ui.ref_sel].as_ref().map_or("-".into(), |r| format!("{}/div", si_short(r.vpd as f64, "V")))
        }
        Item::RefOffset => ui.refs[ui.ref_sel].as_ref().map_or("-".into(), |r| si_short(r.offset as f64, "V")),
        Item::RefSrc => format!("CH{}", ui.ref_src + 1),
        Item::RefShow(i) => match &ui.refs[i] {
            None => "empty".into(),
            Some(r) => {
                let src = if r.label.is_empty() { format!("CH{}", r.src + 1) } else { r.label.clone() };
                format!("{} {}", src, if r.on { "on" } else { "off" })
            }
        },
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::RefSel => {
            let slot = (ui.ref_sel as i32 + d).rem_euclid(NREF as i32) as usize;
            ui.select_workspace_source(WaveId::Ref(slot));
        }
        Item::RefColor => {
            if let Some(r) = &mut ui.refs[ui.ref_sel] {
                r.color = cyc(r.color.min(REF_COLORS.len() - 1), REF_COLORS.len(), d);
            }
        }
        Item::RefVpd => {
            if let Some(r) = &mut ui.refs[ui.ref_sel] {
                let i = VSCALES.iter().position(|&s| s >= r.vpd * 0.999).unwrap_or(VSCALES.len() - 1);
                r.vpd = VSCALES[(i as i32 + d).clamp(0, VSCALES.len() as i32 - 1) as usize];
            }
            ui.measure_refs();
        }
        Item::RefOffset => {
            if let Some(r) = &mut ui.refs[ui.ref_sel] {
                r.offset += 0.1 * r.vpd * d as f32;
            }
        }
        Item::RefSrc => ui.ref_src = src_step(ui.ref_src, NCH, d),
        Item::RefShow(i) => {
            if let Some(r) = &mut ui.refs[i] {
                r.on = !r.on;
            }
        }
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::RefExport => ui.ref_export(),
        Item::RefImport => ui.ref_import(),
        Item::RefLoad => ui.ref_load(),
        Item::RefLabel => match &ui.refs[ui.ref_sel] {
            Some(r) => ui.wlan.osk = Some(wlan::Osk::for_ref(ui.ref_sel, &r.label)),
            None => ui.toast("That slot is empty"),
        },
        Item::RefSave => {
            let c = ui.ref_src;
            let rec = ui.last[0].clone();
            let saved = rec.as_ref().filter(|r| !r.ch[c].is_empty()).map(|r| {
                let (v, dt) = ui.meas_wave(r, c);
                let t0 = if v.len() > r.ch[c].len() { r.raw.as_ref().map(|x| x.t0).unwrap_or(r.t0) } else { r.t0 };
                RefWave {
                    src: c,
                    color: 0,
                    label: String::new(),
                    t0,
                    dt,
                    v: v.into(),
                    vpd: ui.m.ch[c].volts_per_div(),
                    offset: ui.m.ch[c].offset,
                    on: true,
                }
            });
            match saved {
                Some(w) => {
                    let slot = ui.refs.iter().position(|r| r.is_none()).unwrap_or(NREF - 1);
                    ui.m.deskew_ref[slot] = ui.m.deskew_ch[c];
                    ui.refs[slot] = Some(w);
                    ui.toast(&format!("CH{} saved to R{}", c + 1, slot + 1));
                    ui.measure_refs();
                    ui.analyzer_update();
                }
                None => ui.toast("Source channel has no data"),
            }
        }
        Item::RefClear => {
            ui.refs = std::array::from_fn(|_| None);
            ui.measure_refs();
            ui.analyzer_update();
        }
        _ => {}
    }
    true
}

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::RefSrc => return Some(ch_names()),
        Item::RefImport => return Some(ui.ref_files.iter().map(|p| crate::store::file_name(p)).collect()),
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Ref => vec![
            Item::RefSrc,
            Item::RefSave,
            Item::RefClear,
            Item::RefSel,
            Item::RefVpd,
            Item::RefOffset,
            Item::RefColor,
            Item::RefLabel,
            Item::RefLoad,
            Item::RefExport,
            Item::RefImport,
        ]
        .into_iter()
        .filter(|&it| it != Item::RefLoad || ui.feature_on("load"))
        .chain((0..NREF).map(Item::RefShow))
        .collect(),
        _ => Vec::new(),
    }
}
