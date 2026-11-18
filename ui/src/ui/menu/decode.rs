// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area {
    sources,
    kind,
    label,
    value,
    adjust,
    activate,
    options,
    menu_items: |ui, _| ui.decode_items(),
    ..Area::EMPTY
};

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::DecThr => Kind::Value,
        Item::DecBits => Kind::Value,
        Item::DecTable | Item::DecExport => Kind::Action,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::Open(MenuId::Decode) => "Decode",
        Item::DecOn => "Decoder",
        Item::DecSel => "Decoder no.",
        Item::DecTable => "Event table",
        Item::DecExport => "Export events",
        Item::DecSrc => "Source",
        Item::DecBaud => "Baud",
        Item::DecCphyRate => "Symbol rate",
        Item::DecCanRate => "Bit rate",
        Item::DecCanH => "Signal",
        Item::DecUsbSpeed => "Speed",
        Item::DecOpt(_) => "Option",
        Item::DecSrc4 => "Source 4",
        Item::DecDiff => "B line (A-B)",
        Item::DecThr => "Threshold",
        Item::DecFmt => "Format",
        Item::DecProto => "Protocol",
        Item::DecSrc2 => "Clock",
        Item::DecCs => "CS",
        Item::DecMode => "Mode",
        Item::DecBits => "Data bits",
        Item::DecParity => "Parity",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    if let Some(v) = ui.la_value(it) {
        return Some(v);
    }
    let m = &ui.m;
    let onoff = |b: bool| if b { "On".to_string() } else { "Off".to_string() };
    Some(match it {
        Item::DecOn => onoff(m.dec_on),
        Item::DecSel => format!("D{}", m.dec_sel + 1),
        Item::DecSrc => decode_source_name(m.dec_src),
        Item::DecBaud => format!("{}", crate::decode::BAUDS[m.dec_baud]),
        Item::DecCphyRate => crate::decode::cphy_rate_label(m.dec_cphy_rate),
        Item::DecCanRate => si(crate::decode::CAN_RATES[m.dec_canrate], "bit/s"),
        Item::DecCanH => {
            if m.dec_canh {
                "CAN_H".into()
            } else {
                "RX/TX".into()
            }
        }
        Item::DecUsbSpeed => USPEED_NAMES[m.dec_uspeed].into(),
        Item::DecSrc4 => m.dec_src4.map_or("None".into(), decode_source_name),
        Item::DecDiff => m.dec_diff.map_or("None".into(), decode_source_name),
        Item::DecThr => si(m.dec_thr as f64, "V"),
        Item::DecFmt => {
            if m.dec_ascii {
                "ASCII".into()
            } else {
                "Hex".into()
            }
        }
        Item::DecProto => m.dec_proto.name().into(),
        Item::DecOpt(k) => m
            .dec_proto
            .module()
            .and_then(|p| p.options().get(k))
            .map_or(String::new(), |o| o.choices[m.dec_opts()[k]].into()),
        Item::DecSrc2 => decode_source_name(m.dec_src2),
        Item::DecCs => m.dec_cs.map_or("None".into(), decode_source_name),
        Item::DecMode => format!("{}", m.dec_mode),
        Item::DecBits => format!("{}", m.dec_bits),
        Item::DecParity => match m.dec_parity {
            oos_dsp::decode::Parity::None => "None".into(),
            oos_dsp::decode::Parity::Even => "Even".into(),
            oos_dsp::decode::Parity::Odd => "Odd".into(),
        },
        _ => return None,
    })
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    if ui.la_adjust(it, d) {
        return true;
    }
    if adjust_sources(ui, it, d) {
        return false;
    }
    let step = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).clamp(0, n as i32 - 1) as usize };
    let cyc = |i: usize, n: usize, d: i32| -> usize { (i as i32 + d).rem_euclid(n as i32) as usize };
    match it {
        Item::DecOn => ui.m.dec_on = !ui.m.dec_on,
        Item::DecSel => {
            let k = (ui.m.dec_sel as i32 + d).rem_euclid(DECODERS as i32) as usize;
            ui.m.dec_select(k);
        }
        Item::DecBaud => ui.m.dec_baud = step(ui.m.dec_baud, crate::decode::BAUDS.len(), d),
        Item::DecCphyRate => {
            let rates = crate::decode::CPHY_RATES;
            let index = rates.iter().position(|&rate| rate >= ui.m.dec_cphy_rate).unwrap_or(rates.len() - 1);
            ui.m.dec_cphy_rate = rates[cyc(index, rates.len(), d)];
        }
        Item::DecCanRate => ui.m.dec_canrate = step(ui.m.dec_canrate, crate::decode::CAN_RATES.len(), d),
        Item::DecCanH => ui.m.dec_canh = !ui.m.dec_canh,
        Item::DecThr => ui.m.dec_thr += d as f32 * ui.decoder_vpd() / 25.0,
        Item::DecFmt => ui.m.dec_ascii = !ui.m.dec_ascii,
        Item::DecProto => {
            // only the decoders System > Features offers (none: a hand-edited system.conf, see `load_system`)
            let on = ui.protos_on();
            if !on.is_empty() {
                let i = on.iter().position(|&x| x == ui.m.dec_proto).unwrap_or(0);
                ui.set_dec_proto(on[cyc(i, on.len(), d)]);
            }
        }
        Item::DecUsbSpeed => ui.m.dec_uspeed = cyc(ui.m.dec_uspeed, crate::decode::USB_SPEEDS.len(), d),
        Item::DecOpt(k) => {
            if let Some(o) = ui.m.dec_proto.module().and_then(|p| p.options().get(k)) {
                ui.m.dec_opt[k] = cyc(ui.m.dec_opts()[k], o.choices.len(), d) as u8;
            }
        }
        Item::DecMode => ui.m.dec_mode = cyc(ui.m.dec_mode as usize, 4, d) as u8,
        Item::DecBits => {
            let (lo, hi) = if ui.m.dec_proto == Proto::UART { (5, 9) } else { (4, 32) };
            ui.m.dec_bits = (ui.m.dec_bits as i32 + d).clamp(lo, hi) as u32;
        }
        Item::DecParity => {
            use oos_dsp::decode::Parity;
            let all = [Parity::None, Parity::Even, Parity::Odd];
            let i = all.iter().position(|&x| x == ui.m.dec_parity).expect("all parity modes are listed");
            ui.m.dec_parity = all[cyc(i, 3, d)];
        }
        _ => {}
    }
    false
}

fn activate(ui: &mut Ui, it: Item) -> bool {
    match it {
        Item::DecTable => ui.table_toggle(tables::TableKind::Decode),
        Item::DecExport => ui.events_export(tables::TableKind::Decode),
        _ => {}
    }
    true
}

fn options(ui: &Ui, it: Item) -> Option<Vec<String>> {
    if let Some(v) = ui.la_options(it) {
        return Some(v);
    }
    let v: &[&str] = match it {
        Item::DecOn => &["Off", "On"],
        Item::DecSrc => return Some(ui.decoder_sources().into_iter().map(decode_source_name).collect()),
        Item::DecProto => return Some(ui.protos_on().iter().map(|p| p.name().to_string()).collect()),
        Item::DecUsbSpeed => &USPEED_NAMES,
        Item::DecOpt(k) => {
            return Some(
                ui.m.dec_proto
                    .module()
                    .and_then(|p| p.options().get(k))
                    .map_or(Vec::new(), |o| o.choices.iter().map(|c| c.to_string()).collect()),
            )
        }
        Item::DecSrc4 | Item::DecDiff => {
            return Some(
                std::iter::once("None".into())
                    .chain(ui.decoder_sources().into_iter().map(decode_source_name))
                    .collect(),
            )
        }
        Item::DecCanH => &["RX/TX", "CAN_H"],
        Item::DecSrc2 => return Some(ui.decoder_sources().into_iter().map(decode_source_name).collect()),
        Item::DecCs => {
            return Some(
                (if ui.m.dec_proto == Proto::CPHY { Vec::new() } else { vec!["None".into()] })
                    .into_iter()
                    .chain(ui.decoder_sources().into_iter().map(decode_source_name))
                    .collect(),
            )
        }
        Item::DecMode => &["0", "1", "2", "3"],
        Item::DecParity => &["None", "Even", "Odd"],
        Item::DecFmt => &["Hex", "ASCII"],
        Item::DecSel => &["D1", "D2", "D3", "D4"],
        Item::DecBaud => return Some(crate::decode::BAUDS.iter().map(|b| b.to_string()).collect()),
        Item::DecCphyRate => {
            return Some(crate::decode::CPHY_RATES.into_iter().map(crate::decode::cphy_rate_label).collect())
        }
        Item::DecCanRate => return Some(crate::decode::CAN_RATES.iter().map(|&b| si(b, "bit/s")).collect()),
        // the value picker is a hex keypad
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}

pub(in crate::ui) fn decode_items(ui: &Ui) -> Vec<Item> {
    use Item::*;
    let p = ui.m.dec_proto;
    let r = p.roles();
    let on_la = ui.m.dec_on_la();
    let mut v = vec![DecSel, DecOn, DecProto];
    if ui.m.la_on() && p != Proto::CPHY {
        v.push(DecIn);
    }
    v.push(DecSrc);
    if r[1].is_some() {
        v.push(DecSrc2);
    }
    if r[2].is_some() {
        v.push(DecCs);
    }
    if r[3].is_some() {
        v.push(DecSrc4);
    }
    if p.diff_ok() && !on_la {
        v.push(DecDiff);
    }
    // the shared rows the protocol names, then its own options
    use crate::protocols::Row;
    for r in p.module().map_or(&[][..], |x| x.shared()) {
        v.push(match r {
            Row::Baud => DecBaud,
            Row::Bits => DecBits,
            Row::Parity => DecParity,
            Row::SpiMode => DecMode,
            Row::CanRate => DecCanRate,
            Row::CanH => DecCanH,
            Row::UsbSpeed => DecUsbSpeed,
            Row::CphyRate => DecCphyRate,
            Row::Fmt => DecFmt,
        });
    }
    v.extend((0..p.module().map_or(0, |x| x.options().len())).map(DecOpt));
    // LA lines: the probe's comparators set the levels (LA page)
    if !on_la {
        v.push(DecThr);
    }
    // what it decoded, as a table, and as a file
    v.extend([DecTable, DecExport]);
    v
}

fn sources(ui: &Ui, menu: MenuId) -> Vec<WaveId> {
    let m = &ui.m;
    match menu {
        MenuId::Decode if !m.dec_on_la() => decode_wave(m.dec_src).into_iter().collect(),
        _ => Vec::new(),
    }
}

fn adjust_sources(ui: &mut Ui, it: Item, d: i32) -> bool {
    match it {
        Item::DecSrc => {
            let next = ui.decoder_source_step(Some(ui.m.dec_src), false, d);
            ui.configure_decoder_source(0, next);
        }
        Item::DecSrc4 => {
            let next = ui.decoder_source_step(ui.m.dec_src4, true, d);
            ui.configure_decoder_source(3, next);
        }
        Item::DecDiff => {
            let next = ui.decoder_source_step(ui.m.dec_diff, true, d);
            ui.configure_decoder_source(4, next);
        }
        Item::DecSrc2 => {
            let next = ui.decoder_source_step(Some(ui.m.dec_src2), false, d);
            ui.configure_decoder_source(1, next);
        }
        Item::DecCs => {
            let next = ui.decoder_source_step(ui.m.dec_cs, ui.m.dec_proto != Proto::CPHY, d);
            ui.configure_decoder_source(2, next);
        }
        _ => return false,
    }
    true
}
