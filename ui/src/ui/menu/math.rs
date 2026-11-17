// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, activate, options, menu_items, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::MathLinA | Item::MathLinB | Item::MathScale | Item::MathOffset | Item::MathFc | Item::MathBw => {
            Kind::Value
        }
        Item::MathAuto => Kind::Action,
        Item::MathExprEdit => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Math) => "Math",
        Item::MathOpI => "Operation",
        Item::MathFam => "Category",
        Item::MathA => "Source A",
        Item::MathB => "Source B",
        Item::MathScale => "Scale",
        Item::MathOffset => "Offset",
        Item::FftWin => "Window",
        Item::MathFc => "Frequency",
        Item::MathBw => "Bandwidth",
        Item::MathSel => "Channel",
        Item::MathAuto => "Auto scale",
        Item::MathExprI => "Formula",
        Item::MathExprEdit => "Edit formula",
        Item::MathLinA => "A",
        Item::MathLinB => "B",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    let m = &ui.m;
    Some(match it {
        Item::MathExprI => {
            if m.math_expr[m.math_sel].is_empty() {
                "none".to_string()
            } else {
                m.math_expr[m.math_sel].clone()
            }
        }
        Item::MathLinA => format!("{:.3}", m.math_lin.0),
        Item::MathLinB => format!("{:.3}", m.math_lin.1),
        Item::MathOpI => m.math.name().into(),
        Item::MathFam => ui.math_fam.name().into(),
        Item::MathA => src_name(m.math_a),
        Item::MathB => src_name(m.math_b),
        Item::MathSel => format!("M{}", m.math_sel + 1),
        Item::MathBw => si_short(m.math_bw as f64, "Hz"),
        Item::MathScale => ui.math_scale_text(),
        Item::MathOffset => format!("{:.2} div", m.math_offset),
        Item::FftWin => m.fft_win.name().into(),
        Item::MathFc => si_short(m.math_fc as f64, "Hz"),
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    if adjust_operation(ui, it, d) {
        return false;
    }
    if matches!(it, Item::MathA | Item::MathB) {
        let sources = ui.math_source_ids(ui.m.math_sel);
        let current = if it == Item::MathA { ui.m.math_a } else { ui.m.math_b };
        if !sources.is_empty() {
            let i = sources.iter().position(|&s| s == current).unwrap_or(0);
            let source = sources[(i as i32 + d).rem_euclid(sources.len() as i32) as usize];
            if it == Item::MathA {
                ui.m.math_a = source;
            } else {
                ui.m.math_b = source;
            }
            change::edited(ui);
        }
        return true;
    }
    let m = &mut ui.m;
    let step = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).clamp(0, n as i32 - 1) as usize };
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::MathLinA => m.math_lin.0 = (m.math_lin.0 + 0.1 * d as f32).clamp(-1e6, 1e6),
        Item::MathLinB => m.math_lin.1 = (m.math_lin.1 + 0.1 * d as f32).clamp(-1e6, 1e6),
        Item::MathBw => {
            let l = time_ladder();
            let per = 1.0 / m.math_bw.max(1e-6);
            let i = l.iter().position(|&x| x >= per * 0.999).unwrap_or(0);
            m.math_bw = 1.0 / l[step(i, l.len(), -d)];
        }
        Item::MathScale => {
            let n = if m.math == MathOp::Fft { FFT_DB.len() } else { math_scales().len() };
            m.math_scale = step(m.math_scale, n, d);
        }
        Item::MathOffset => m.math_offset += d as f32 * 0.1,
        Item::FftWin => {
            let i = WINDOWS.iter().position(|&w| w == m.fft_win).unwrap_or(0);
            m.fft_win = WINDOWS[cyc(i, WINDOWS.len(), d)];
        }
        Item::MathFc => {
            let l = time_ladder();
            let per = 1.0 / m.math_fc.max(1e-6);
            let i = l.iter().position(|&x| x >= per * 0.999).unwrap_or(0);
            m.math_fc = 1.0 / l[step(i, l.len(), -d)];
        }
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::MathExprEdit => ui.wlan.osk = Some(wlan::Osk::for_expr(ui.m.math_sel, &ui.m.math_expr[ui.m.math_sel])),
        Item::MathAuto => ui.math_autoscale(),
        _ => {}
    }
    true
}

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::MathA | Item::MathB => {
            return Some(ui.math_source_ids(ui.m.math_sel).into_iter().map(src_name).collect())
        }
        Item::MathSel => return Some((0..NMATH).map(|k| format!("M{}", k + 1)).collect()),
        Item::FftWin => &["Rect", "Hann", "Hamming", "Blackman", "Flattop"],
        // long lists: shown as a picker (see `ctl`)
        Item::MathOpI => return Some(ui.math_ops_in_family().iter().map(|o| o.name().to_string()).collect()),
        Item::MathFam => return Some(MathFamily::ALL.iter().map(|f| f.name().to_string()).collect()),
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

fn menu_items(ui: &Ui, menu: MenuId) -> Vec<Item> {
    match menu {
        MenuId::Math if ui.m.math == MathOp::Off => vec![Item::MathSel, Item::MathFam, Item::MathOpI],
        MenuId::Math if ui.m.math == MathOp::Expr => vec![
            Item::MathSel,
            Item::MathFam,
            Item::MathOpI,
            Item::MathExprI,
            Item::MathExprEdit,
            Item::MathScale,
            Item::MathOffset,
            Item::MathAuto,
        ],
        MenuId::Math => {
            let mut v = vec![Item::MathSel, Item::MathFam, Item::MathOpI, Item::MathA];
            if ui.m.math.two_sources() {
                v.push(Item::MathB);
            }
            if ui.m.math == MathOp::Lin {
                v.extend([Item::MathLinA, Item::MathLinB]);
            }
            if ui.m.math.has_fc() {
                v.push(Item::MathFc);
            }
            if ui.m.math.has_bw() {
                v.push(Item::MathBw);
            }
            v.extend([Item::MathScale, Item::MathOffset]);
            if ui.m.math == MathOp::Fft {
                v.push(Item::FftWin);
            }
            if ui.m.math != MathOp::Off {
                v.push(Item::MathAuto);
            }
            v
        }
        _ => Vec::new(),
    }
}

fn adjust_operation(ui: &mut Ui, it: Item, d: i32) -> bool {
    let m = &mut ui.m;
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::MathOpI => {
            let fam = ui.math_fam;
            let mut ops = vec![MathOp::Off];
            ops.extend_from_slice(fam.ops());
            let i = ops.iter().position(|&x| x == m.math).unwrap_or(0);
            m.math = ops[cyc(i, ops.len(), d)];
            m.math_offset = 0.0;
            ui.math_autoscale();
        }
        Item::MathFam => {
            let i = MathFamily::ALL.iter().position(|&f| f == ui.math_fam).unwrap_or(0);
            ui.math_fam = MathFamily::ALL[cyc(i, MathFamily::ALL.len(), d)];
            if m.math != MathOp::Off && m.math.family() != ui.math_fam {
                m.math = ui.math_fam.ops()[0];
                m.math_offset = 0.0;
                ui.math_autoscale();
            }
        }
        // sources: CH1..4 and the math channels below the selected one
        Item::MathSel => {
            let k = cyc(m.math_sel, NMATH, d);
            m.math_select(k);
            if m.math != MathOp::Off {
                ui.math_fam = m.math.family();
            }
        }
        _ => return false,
    }
    true
}
