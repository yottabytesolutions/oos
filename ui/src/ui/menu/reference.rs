// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust, activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::action(Open(MenuId::Ref), "Reference").when(|_| false),
        Descriptor::choice(RefSrc, "Save from", |u| format!("CH{}", u.ref_src + 1), |_| ch_names()),
        Descriptor::action(RefSave, "Save to next R"),
        Descriptor::action(RefClear, "Clear all"),
        Descriptor::value(RefSel, "Slot", |u| format!("R{}", u.ref_sel + 1)),
        Descriptor::value(RefVpd, "Slot scale", |u| {
            u.refs[u.ref_sel].as_ref().map_or("-".into(), |r| format!("{}/div", si_short(r.vpd as f64, "V")))
        }),
        Descriptor::value(RefOffset, "Slot offset", |u| {
            u.refs[u.ref_sel].as_ref().map_or("-".into(), |r| si_short(r.offset as f64, "V"))
        }),
        Descriptor::value(RefColor, "Slot color", |u| {
            u.refs[u.ref_sel].as_ref().map_or("-", |r| REF_COLORS[r.color.min(REF_COLORS.len() - 1)].0).into()
        }),
        Descriptor::action(RefLabel, "Slot name"),
        Descriptor::action(RefLoad, "Load file").when(|u| u.feature_on("load")),
        Descriptor::action(RefExport, "Export slot"),
        Descriptor::new(
            RefImport,
            Kind::Action,
            "Import slot",
            |_| String::new(),
            |u| u.ref_files.iter().map(|p| crate::store::file_name(p)).collect(),
            |_| true,
        ),
        slot::<0>(),
        slot::<1>(),
        slot::<2>(),
        slot::<3>(),
        slot::<4>(),
        slot::<5>(),
        slot::<6>(),
        slot::<7>(),
        slot::<8>(),
        slot::<9>(),
    ]
};

const fn slot<const N: usize>() -> Descriptor {
    Descriptor::choice(
        Item::RefShow(N),
        ["R1", "R2", "R3", "R4", "R5", "R6", "R7", "R8", "R9", "R10"][N],
        |u| {
            u.refs[N].as_ref().map_or("empty".into(), |r| {
                let src = if r.label.is_empty() { format!("CH{}", r.src + 1) } else { r.label.clone() };
                format!("{} {}", src, if r.on { "on" } else { "off" })
            })
        },
        |_| Vec::new(),
    )
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
