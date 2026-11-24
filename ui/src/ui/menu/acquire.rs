// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;
use crate::model::AcqMode as AcquisitionMode;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust, activate, ..Area::EMPTY };

const AUTO_DEPTH_LABELS: [&str; 2] = ["Extended", "Screen"];

const ROWS: &[Descriptor] = {
    use Item::*;
    use Kind::*;
    &[
        Descriptor::new(Open(MenuId::Acquire), Action, "Acquire", |_| String::new(), |_| Vec::new(), |_| false),
        Descriptor::new(
            AcqMode,
            Choice,
            "Mode",
            |u| {
                match u.m.acq {
                    AcquisitionMode::Normal => "Normal",
                    AcquisitionMode::Peak => "Peak",
                    AcquisitionMode::Average => "Average",
                    AcquisitionMode::HiRes => "High res",
                }
                .into()
            },
            |_| ["Normal", "Peak", "Average", "High res"].into_iter().map(String::from).collect(),
            |_| true,
        ),
        Descriptor::new(
            AcqAvg,
            Value,
            "Averages",
            |u| u.m.avg_count.to_string(),
            |_| AVERAGES.iter().map(ToString::to_string).collect(),
            |u| u.m.acq == AcquisitionMode::Average,
        ),
        Descriptor::new(
            AcqAvgHw,
            Choice,
            "HW average",
            |u| onoff(u.m.avg_hw),
            toggles,
            |u| u.m.acq == AcquisitionMode::Average && u.src.is_hw(),
        ),
        Descriptor::new(
            AcqDepth,
            Choice,
            "Memory",
            |u| mdepths()[u.m.mdepth].0.into(),
            |_| mdepths().iter().map(|d| d.0.to_string()).collect(),
            |_| true,
        ),
        Descriptor::new(
            AcqAutoDepth,
            Choice,
            "Auto memory",
            |u| AUTO_DEPTH_LABELS[auto_depth_index(u.m.auto_acquisition.depth())].into(),
            |_| AUTO_DEPTH_LABELS.into_iter().map(String::from).collect(),
            |u| u.src.is_hw() && u.m.mdepth == 0,
        ),
        Descriptor::new(
            AcqSeg,
            Choice,
            "Segments",
            |u| if u.m.segments == 0 { "Off".into() } else { u.seg_count().to_string() },
            |u| u.seg_options().iter().map(|&n| if n == 0 { "Off".into() } else { n.to_string() }).collect(),
            |_| true,
        ),
        Descriptor::new(AcqSegHw, Choice, "HW segments", |u| onoff(u.m.seg_hw), toggles, |u| u.m.segments > 0),
        Descriptor::new(
            AcqSegIvl,
            Choice,
            "Seg. interval",
            |u| seg_ivl_name(u.m.seg_ivl),
            |_| SEG_IVLS.iter().map(|&x| seg_ivl_name(x)).collect(),
            Ui::seg_hw_on,
        ),
        Descriptor::new(AcqSegTable, Action, "Segment times", |_| String::new(), |_| Vec::new(), Ui::seg_hw_on),
        Descriptor::new(
            AcqCores,
            Choice,
            "ADC cores",
            |u| if u.m.cores { "Matched" } else { "Raw" }.into(),
            |_| ["Raw", "Matched"].into_iter().map(String::from).collect(),
            |_| true,
        ),
        Descriptor::new(
            AcqAlias,
            Choice,
            "Aliasing",
            |u| u.alias_above().map_or(String::new(), |f| format!("above {}, Peak shows it", si_short(f, "Hz"))),
            |_| Vec::new(),
            |u| u.alias_above().is_some(),
        ),
        Descriptor::new(Open(MenuId::Horizontal), Action, "Horizontal", |_| String::new(), |_| Vec::new(), |_| true),
    ]
};

fn auto_depth_index(depth: AutoDepth) -> usize {
    AutoDepth::ALL.iter().position(|&choice| choice == depth).expect("all Auto memory policies are listed")
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
        Item::AcqAutoDepth => {
            let i = auto_depth_index(m.auto_acquisition.depth());
            m.auto_acquisition.set_depth(AutoDepth::ALL[cyc(i, AutoDepth::ALL.len(), d)]);
        }
        Item::AcqCores => m.cores = !m.cores,
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
