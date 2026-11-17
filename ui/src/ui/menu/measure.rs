// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    if let Some(v) = Ui::meas_kind(it) {
        return Some(v);
    }
    Some(match it {
        Item::MeasPair(_)
        | Item::MeasAdd(_)
        | Item::AddMeas
        | Item::MeasCfg(_)
        | Item::MeasRemove(_)
        | Item::MeasClear => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    if let Some(v) = Ui::meas_cfg_label(it) {
        return Some(v);
    }
    Some(match it {
        Item::Open(MenuId::Measure) => "Measure",
        Item::AddMeas => "Add meas",
        Item::Open(MenuId::MeasPair) => "Channel pair",
        Item::MeasStats => "Statistics",
        Item::MeasLabels => "Labels on trace",
        Item::StripDock => "Dock",
        Item::Open(MenuId::MeasSelected) => "Selected",
        Item::MeasSrc => "Source A",
        Item::MeasCopyTo => "Copy meas. to",
        Item::MeasSrcB => "Source B",
        Item::MeasPair(k) => tools::PAIRS[k.min(tools::PAIRS.len() - 1)].0,
        Item::MeasAdd(k) => k.name(),
        Item::MeasCfg(_) => "Set",
        Item::MeasClear => "Clear all",
        Item::MeasRemove(_) => "Remove",
        Item::CatSourceC => "Source C",
        Item::CatSourceD => "Source D",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    if it == Item::CatSourceC {
        return Some(ui.wave_name(ui.cat_c));
    }
    if it == Item::CatSourceD {
        return Some(ui.wave_name(ui.cat_d));
    }
    if let Some(v) = ui.meas_value(it) {
        return Some(v);
    }
    let onoff = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    Some(match it {
        Item::MeasSrc => ui.wave_name(ui.meas_src),
        Item::MeasCopyTo => "Pick".into(),
        Item::MeasSrcB => ui.wave_name(ui.meas_src_b),
        Item::MeasStats => onoff(ui.stats_on),
        Item::MeasLabels => onoff(ui.m.meas_labels),
        Item::StripDock => ui.m.strip_dock.label().into(),
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    if matches!(it, Item::CatSourceC | Item::CatSourceD) {
        let current = if it == Item::CatSourceC { ui.cat_c } else { ui.cat_d };
        let next = ui.step_wave(current, d);
        if it == Item::CatSourceC {
            ui.cat_c = next;
        } else {
            ui.cat_d = next;
        }
        return true;
    }
    if ui.meas_adjust(it, d) {
        return true;
    }
    let m = &mut ui.m;
    match it {
        Item::MeasSrc => {
            let cur = ui.meas_src;
            ui.meas_src = ui.step_wave(cur, d);
        }
        Item::MeasSrcB => {
            let cur = ui.meas_src_b;
            ui.meas_src_b = ui.step_wave(cur, d);
        }
        // Track cursor B on its own source (follows cursor A's until set)
        Item::MeasStats => ui.set_stats_shown(!ui.stats_on),
        Item::MeasLabels => ui.m.meas_labels = !ui.m.meas_labels,
        Item::StripDock => {
            let all = StripDock::ALL;
            let i = all.iter().position(|&s| s == m.strip_dock).unwrap_or(0);
            m.strip_dock = all[(i as i32 + d).clamp(0, all.len() as i32 - 1) as usize];
            // the statistics card follows the dock the strip uses
            ui.workspace.measure_docked = true;
            ui.workspace.measure_float = None;
            // moving the strip does not change what was measured
            ui.relayout();
            return true;
        }
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::AddMeas => {
            if let Some(k) = ui.next_usual_meas() {
                ui.activate(Item::MeasAdd(k));
            }
        }
        Item::MeasCfg(i) => ui.open_meas_cfg(i as usize),
        Item::MeasAdd(k) => {
            let e = (k, ui.meas_src);
            if let Some(index) = ui.m.meas.iter().position(|item| *item == e) {
                ui.m.meas.remove(index);
                ui.meas_own.remove(&e);
                if ui.meas_cfg == Some(index as u16) {
                    ui.meas_cfg = None;
                } else if ui.meas_cfg.is_some_and(|n| n as usize > index) {
                    ui.meas_cfg = ui.meas_cfg.map(|n| n - 1);
                }
                ui.toast(&format!("{} {} removed", meascatalog::meas_long_name(k), ui.wave_name(ui.meas_src)));
                return true;
            } else {
                if !ui.meas_make_room() {
                    return true;
                }
                ui.m.meas.push(e);
            }
            if k == Meas::Burst && ui.m.meas_idle.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
                if let Some((a, _)) = ui.meas_analysis(ui.meas_src) {
                    let p = a.period();
                    if p.is_finite() && p > 0.0 {
                        ui.m.meas_idle = p;
                    }
                }
            }
            ui.ensure_meas_own();
            ui.toast(&format!("{} {} added", meascatalog::meas_long_name(k), ui.wave_name(ui.meas_src)));
        }
        Item::MeasPair(k) => ui.meas_pair_toggle(k),
        Item::MeasRemove(index) => ui.remove_measurement(index as usize),
        Item::MeasClear => {
            ui.m.meas.clear();
            ui.m.meas2.clear();
            ui.m.cat.clear();
            ui.meas_own.clear();
            ui.meas_cfg = None;
            change::stats_reset(ui);
        }
        _ => {}
    }
    true
}

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    if let Some(v) = ui.meas_options(it) {
        return Some(v);
    }
    let v: &[&str] = match it {
        Item::StripDock => return Some(StripDock::ALL.iter().map(|s| s.label().to_string()).collect()),
        Item::MeasStats | Item::MeasLabels => &["Off", "On"],
        Item::MeasSrc | Item::MeasSrcB | Item::CatSourceC | Item::CatSourceD => return Some(ui.wave_names()),
        Item::MeasCopyTo => {
            return Some(ui.wave_names().into_iter().filter(|n| *n != ui.wave_name(ui.meas_src)).collect())
        }
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::MeasPair => (0..tools::PAIRS.len()).map(Item::MeasPair).collect(),
        MenuId::Measure => vec![
            Item::MeasSrc,
            Item::MeasSrcB,
            Item::MeasCopyTo,
            Item::MeasGate,
            Item::MeasRefs,
            Item::MeasStats,
            Item::MeasLabels,
            Item::StripDock,
            Item::MeasIdle,
            Item::MeasTsetup,
            Item::MeasThold,
            Item::MeasOutside,
            Item::MeasN,
            Item::MeasEdges,
            Item::Open(MenuId::MeasPair),
            Item::Open(MenuId::Dvm),
            Item::MeasClear,
        ],
        MenuId::MeasHold => vec![Item::AddMeas],
        MenuId::MeasAdd => Vec::new(),
        MenuId::MeasSelected => ui.measurement_catalog(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burst_idle_uses_the_period_unless_already_positive() {
        let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
        ui.refs[0] = Some(RefWave {
            src: 0,
            color: 0,
            label: String::new(),
            t0: 0.0,
            dt: 1e-9,
            v: (0..400).map(|i| if (i / 20) % 2 == 0 { -1.0 } else { 1.0 }).collect::<Vec<_>>().into(),
            vpd: 1.0,
            offset: 0.0,
            on: true,
        });
        ui.meas_src = WaveId::Ref(0);
        let period = ui.meas_analysis(ui.meas_src).unwrap().0.period();
        assert!(period.is_finite() && period > 0.0);
        for idle in [f64::NAN, f64::NEG_INFINITY, -1.0, -0.0, 0.0, 1.0, f64::INFINITY] {
            ui.m.meas.clear();
            ui.meas_own.clear();
            ui.m.meas_idle = idle;
            activate(&mut ui, Item::MeasAdd(Meas::Burst));
            assert_eq!(ui.m.meas_idle, if idle > 0.0 { idle } else { period });
        }
    }
}
