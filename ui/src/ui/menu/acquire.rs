// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::AcqAvg => Kind::Value,
        Item::AcqSegTable => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Acquire) => "Acquire",
        Item::AcqMode => "Mode",
        Item::AcqAvg => "Averages",
        Item::AcqSeg => "Segments",
        Item::AcqSegHw => "HW segments",
        Item::AcqAvgHw => "HW average",
        Item::AcqSegIvl => "Seg. interval",
        Item::AcqSegTable => "Segment times",
        Item::AcqDepth => "Memory",
        Item::AcqCores => "ADC cores",
        Item::AcqDrawPath => "Draw path",
        Item::AcqAlias => "Aliasing",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    let m = &ui.m;
    let onoff = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    Some(match it {
        Item::AcqMode => match m.acq {
            AcqMode::Normal => "Normal".into(),
            AcqMode::Peak => "Peak".into(),
            AcqMode::Average => "Average".into(),
            AcqMode::HiRes => "High res".into(),
        },
        Item::AcqAvg => format!("{}", m.avg_count),
        Item::AcqSeg => {
            if m.segments == 0 {
                "Off".into()
            } else {
                ui.seg_count().to_string()
            }
        }
        Item::AcqSegHw => onoff(m.seg_hw),
        Item::AcqAvgHw => onoff(m.avg_hw),
        Item::AcqSegIvl => seg_ivl_name(m.seg_ivl),
        Item::AcqDepth => mdepths()[m.mdepth].0.into(),
        Item::AcqCores => if m.cores { "Matched" } else { "Raw" }.into(),
        Item::AcqDrawPath => if m.draw_batch { "Batch" } else { "Standard" }.into(),
        Item::AcqAlias => {
            ui.alias_above().map_or(String::new(), |f| format!("above {}, Peak shows it", si_short(f, "Hz")))
        }
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    if seg_adjust(ui, it, d) {
        return true;
    }
    let m = &mut ui.m;
    let step = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).clamp(0, n as i32 - 1) as usize };
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::AcqMode => {
            let all = [AcqMode::Normal, AcqMode::Peak, AcqMode::Average, AcqMode::HiRes];
            let i = all.iter().position(|&x| x == m.acq).expect("all acquisition modes are listed");
            m.acq = all[cyc(i, 4, d)];
        }
        Item::AcqAvg => {
            let i = AVERAGES.iter().position(|&x| x == m.avg_count).unwrap_or(3);
            m.avg_count = AVERAGES[step(i, AVERAGES.len(), d)];
        }
        Item::AcqDepth => m.mdepth = cyc(m.mdepth, mdepths().len(), d),
        Item::AcqCores => m.cores = !m.cores,
        Item::AcqDrawPath => {
            m.draw_batch = !m.draw_batch;
            change::mapping_observed(ui, change::oracle::Row::Layout);
        }
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    if it == Item::AcqSegTable {
        ui.table_toggle(tables::TableKind::Segments);
    }
    true
}

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::AcqMode => &["Normal", "Peak", "Average", "High res"],
        Item::AcqCores => &["Raw", "Matched"],
        Item::AcqDrawPath => &["Standard", "Batch"],
        Item::AcqSegHw | Item::AcqAvgHw => &["Off", "On"],
        Item::AcqDepth => return Some(mdepths().iter().map(|d| d.0.to_string()).collect()),
        Item::AcqAvg => return Some(AVERAGES.iter().map(|n| n.to_string()).collect()),
        Item::AcqSeg => {
            return Some(ui.seg_options().iter().map(|&n| if n == 0 { "Off".into() } else { n.to_string() }).collect())
        }
        Item::AcqSegIvl => return Some(SEG_IVLS.iter().map(|&x| seg_ivl_name(x)).collect()),
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Acquire => {
            let mut v = vec![Item::AcqMode];
            if ui.m.acq == AcqMode::Average {
                v.push(Item::AcqAvg);
                if ui.src.is_hw() {
                    v.push(Item::AcqAvgHw);
                }
            }
            v.push(Item::AcqDepth);
            v.push(Item::AcqSeg);
            if ui.m.segments > 0 {
                v.push(Item::AcqSegHw);
            }
            if ui.seg_hw_on() {
                v.extend([Item::AcqSegIvl, Item::AcqSegTable]);
            }
            v.push(Item::AcqCores);
            v.push(Item::AcqDrawPath);
            if ui.alias_above().is_some() {
                v.push(Item::AcqAlias);
            }
            v.push(Item::Open(MenuId::Horizontal));
            v
        }
        _ => Vec::new(),
    }
}

/// The segment items of the Acquire menu; false for any other item.
fn seg_adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::AcqSeg => {
            let o = ui.seg_options();
            let cur = if ui.m.segments == 0 { 0 } else { ui.seg_count() };
            let i = o.iter().position(|&n| n >= cur).unwrap_or(0);
            ui.m.segments = o[cyc(i, o.len(), d)];
        }
        Item::AcqAvgHw => {
            ui.m.avg_hw = !ui.m.avg_hw;
            if ui.m.avg_hw && !board().avg_hw(&ui.m) {
                ui.toast("HW average needs a count of 2..1024 and ordinary records: software average");
            }
        }
        Item::AcqSegHw => {
            ui.m.seg_hw = !ui.m.seg_hw;
            // the other limit applies from now on
            ui.m.segments = ui.seg_count();
        }
        Item::AcqSegIvl => {
            let i = SEG_IVLS.iter().position(|&x| x >= ui.m.seg_ivl).unwrap_or(0);
            ui.m.seg_ivl = SEG_IVLS[cyc(i, SEG_IVLS.len(), d)];
        }
        _ => return false,
    }
    true
}
