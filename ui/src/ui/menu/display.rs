// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::Intensity | Item::GridBright => Kind::Value,
        Item::ClearDisp => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Display) => "Display",
        Item::ColorGrade => "Colour grade",
        Item::PaneSel => "Lower",
        Item::TrendSel(0) => "Trend chart 1",
        Item::TrendSel(_) => "Trend chart 2",
        Item::XyMode => "Format",
        Item::Persist => "Persistence",
        Item::Intensity => "Intensity",
        Item::GridStyle => "Grid",
        Item::GridBright => "Grid bright.",
        Item::GridFrame => "Grid frame",
        Item::ClearDisp => "Clear",
        Item::Dots => "Dots",
        Item::BottomBar => "Bottom bar",
        Item::Hdmi => "HDMI",
        Item::TextSize => "Text size",
        Item::ZInput => "Z input",
        Item::Histogram => "Histogram",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    let m = &ui.m;
    let onoff = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    Some(match it {
        Item::Persist => PERSIST[m.persist].0.into(),
        Item::Intensity => format!("{} %", m.intensity),
        Item::GridStyle => format!("{:?}", m.grid),
        Item::GridBright => format!("{} %", m.grid_bright),
        Item::GridFrame => onoff(m.grid_frame),
        Item::Dots => format!("{:?}", m.dots),
        Item::BottomBar => if m.bar_compact { "Compact" } else { "Cards" }.into(),
        Item::Hdmi => format!("{:?}", m.hdmi),
        Item::TextSize => m.text_size.name().into(),
        Item::ZInput => m.z_src.map_or("Off".to_string(), |c| format!("CH{}", c + 1)),
        Item::Histogram => format!("{:?}", m.hist),
        Item::PaneSel => tables::PANES[m.pane.min(tables::PANES.len() - 1)].into(),
        Item::TrendSel(k) => ui.trend_options().get(m.trend_sel[k]).cloned().unwrap_or_else(|| "Auto".into()),
        Item::XyMode => {
            if ui.xy {
                "XY".into()
            } else {
                "YT".into()
            }
        }
        Item::ColorGrade => onoff(m.color_grade),
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    if adjust_pane(ui, it, d) {
        return false;
    }
    let m = &mut ui.m;
    let step = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).clamp(0, n as i32 - 1) as usize };
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::Persist => {
            m.persist = cyc(m.persist, PERSIST.len(), d);
        }
        Item::Intensity => m.intensity = (m.intensity as i32 + d * 5).clamp(10, 100) as u32,
        Item::Dots => {
            let i = DOTS_MODES.iter().position(|&x| x == m.dots).unwrap_or(0);
            m.dots = DOTS_MODES[cyc(i, DOTS_MODES.len(), d)];
            change::mapping_observed(ui, change::oracle::Row::Layout);
        }
        Item::BottomBar => {
            m.bar_compact = !m.bar_compact;
            change::mapping_observed(ui, change::oracle::Row::Layout);
        }
        Item::Hdmi => {
            let i = HDMI_MODES.iter().position(|&x| x == m.hdmi).unwrap_or(0);
            m.hdmi = HDMI_MODES[step(i, HDMI_MODES.len(), d)];
            if ui.m.hdmi == HdmiMode::Layout {
                ui.toast("HDMI Layout: a second picture each monitor frame, the panel may get fewer frames");
            }
            change::edited(ui);
            ui.sync_full();
            return true;
        }
        Item::TextSize => {
            let i = m.text_size.index();
            m.text_size = TEXT_SIZES[(i as i32 + d).clamp(0, TEXT_SIZES.len() as i32 - 1) as usize];
        }
        Item::ZInput => {
            let i = m.z_src.map_or(0, |c| c + 1);
            let i = opt_src_step(i, d);
            m.z_src = if i == 0 { None } else { Some(i - 1) };
            change::mapping_observed(ui, change::oracle::Row::Layout);
        }
        Item::Histogram => {
            let all = [Hist::Off, Hist::Vertical, Hist::Horizontal];
            let i = all.iter().position(|&h| h == m.hist).unwrap_or(0);
            m.hist = all[cyc(i, 3, d)];
        }
        Item::GridBright => m.grid_bright = (m.grid_bright as i32 + 10 * d).clamp(10, 100) as u32,
        Item::GridFrame => m.grid_frame = !m.grid_frame,
        Item::GridStyle => {
            let all = [Grid::Full, Grid::Light, Grid::None];
            let i = all.iter().position(|&x| x == m.grid).expect("all grid styles are listed");
            m.grid = all[cyc(i, 3, d)];
        }
        Item::XyMode => ui.xy = !ui.xy,
        Item::ColorGrade => ui.m.color_grade = !ui.m.color_grade,
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    if it == Item::ClearDisp {
        if ui.menu == MenuId::Main {
            ui.close_menu();
        }
        ui.clear_display();
    }
    true
}

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::ColorGrade => &["Off", "On"],
        Item::GridStyle => &["Full", "Light", "None"],
        Item::GridFrame => &["Off", "On"],
        Item::Dots => &["Off", "On", "Auto"],
        Item::BottomBar => &["Cards", "Compact"],
        Item::Hdmi => &["Mirror", "Waveform", "Layout"],
        Item::TextSize => return Some(TEXT_SIZES.iter().map(|t| t.name().to_string()).collect()),
        Item::ZInput => return Some(opt_ch_names("Off")),
        Item::Histogram => &["Off", "Vertical", "Horizontal"],
        Item::XyMode => &["YT", "XY"],
        Item::PaneSel => &tables::PANES,
        Item::TrendSel(_) => return Some(ui.trend_options()),
        Item::Persist => return Some(PERSIST.iter().map(|p| p.0.to_string()).collect()),
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Display => {
            let mut v = vec![
                Item::Open(MenuId::Windows),
                Item::Views,
                Item::Persist,
                Item::ColorGrade,
                Item::Intensity,
                Item::GridStyle,
                Item::GridBright,
                Item::GridFrame,
                Item::Dots,
                Item::BottomBar,
                Item::ZInput,
                Item::Histogram,
                Item::XyMode,
                Item::Hdmi,
                Item::HdmiView,
                Item::Backlight,
                Item::TextSize,
                Item::ClearDisp,
            ];
            if ui.m.pane == tables::PANE_TREND {
                v.splice(1..1, [Item::TrendSel(0), Item::TrendSel(1)]);
            }
            v
        }
        _ => Vec::new(),
    }
}

fn adjust_pane(ui: &mut Ui, it: Item, d: i32) -> bool {
    match it {
        Item::PaneSel => {
            ui.m.pane = (ui.m.pane as i32 + d).rem_euclid(tables::PANES.len() as i32) as usize;
            if ui.m.density == Density::Split {
                ui.m.split_pane = ui.m.pane;
                let prev = ui.m.win_layout;
                ui.m.win_layout = if ui.m.pane == 0 { WinLayout::Rows } else { WinLayout::One };
                if ui.m.win_layout != prev && ui.m.win_layout != WinLayout::One {
                    ui.spread_wins();
                    if ui.m.zoom {
                        ui.toast("Windows show while zoom is off");
                    }
                }
            } else if ui.m.pane != 0 && ui.pane_h() == 0 {
                ui.toast("No room for a second pane here: fewer LA lanes or the scope view");
            }
        }
        Item::TrendSel(k) => {
            let n = ui.trend_options().len() as i32;
            ui.m.trend_sel[k] = (ui.m.trend_sel[k].min(n as usize - 1) as i32 + d).rem_euclid(n) as usize;
        }
        _ => return false,
    }
    true
}
