// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { descriptors: ROWS, ordered_rows, sources, adjust, activate, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::action(Open(MenuId::Decode), "Decode").when(|_| false),
        Descriptor::choice(
            DecSel,
            "Decoder no.",
            |u| format!("D{}", u.m.dec_sel + 1),
            |_| ["D1", "D2", "D3", "D4"].into_iter().map(String::from).collect(),
        ),
        Descriptor::choice(DecOn, "Decoder", |u| onoff(u.m.dec_on), toggles),
        Descriptor::choice(
            DecProto,
            "Protocol",
            |u| u.m.dec_proto.name().into(),
            |u| u.protos_on().iter().map(|p| p.name().to_string()).collect(),
        ),
        Descriptor::choice(DecSrc, "Source", |u| source_value(u, 0, Some(u.m.dec_src)), |u| source_options(u, false)),
        Descriptor::choice(DecSrc2, "Clock", |u| source_value(u, 1, Some(u.m.dec_src2)), |u| source_options(u, false))
            .when(|u| u.m.dec_proto.roles()[1].is_some()),
        Descriptor::choice(
            DecCs,
            "CS",
            |u| source_value(u, 2, u.m.dec_cs),
            |u| source_options(u, u.m.dec_proto != Proto::CPHY),
        )
        .when(|u| u.m.dec_proto.roles()[2].is_some()),
        Descriptor::choice(DecSrc4, "Source 4", |u| source_value(u, 3, u.m.dec_src4), |u| source_options(u, true))
            .when(|u| u.m.dec_proto.roles()[3].is_some()),
        Descriptor::choice(
            DecDiff,
            "B line (A-B)",
            |u| u.m.dec_diff.map_or("None".into(), decode_source_name),
            |u| analog_sources(u, true),
        )
        .when(|u| u.m.dec_proto.diff_ok() && !u.m.dec_on_la()),
        Descriptor::choice(
            DecBaud,
            "Baud",
            |u| crate::decode::BAUDS[u.m.dec_baud].to_string(),
            |_| crate::decode::BAUDS.iter().map(ToString::to_string).collect(),
        ),
        Descriptor::value(DecBits, "Data bits", |u| u.m.dec_bits.to_string()),
        Descriptor::choice(
            DecParity,
            "Parity",
            |u| {
                match u.m.dec_parity {
                    oos_dsp::decode::Parity::None => "None",
                    oos_dsp::decode::Parity::Even => "Even",
                    oos_dsp::decode::Parity::Odd => "Odd",
                }
                .into()
            },
            |_| ["None", "Even", "Odd"].into_iter().map(String::from).collect(),
        ),
        Descriptor::choice(
            DecMode,
            "Mode",
            |u| u.m.dec_mode.to_string(),
            |_| ["0", "1", "2", "3"].into_iter().map(String::from).collect(),
        ),
        Descriptor::choice(
            DecCanRate,
            "Bit rate",
            |u| si(crate::decode::CAN_RATES[u.m.dec_canrate], "bit/s"),
            |_| crate::decode::CAN_RATES.iter().map(|&b| si(b, "bit/s")).collect(),
        ),
        Descriptor::choice(
            DecCanH,
            "Signal",
            |u| if u.m.dec_canh { "CAN_H" } else { "RX/TX" }.into(),
            |_| ["RX/TX", "CAN_H"].into_iter().map(String::from).collect(),
        ),
        Descriptor::choice(
            DecUsbSpeed,
            "Speed",
            |u| USPEED_NAMES[u.m.dec_uspeed].into(),
            |_| USPEED_NAMES.into_iter().map(String::from).collect(),
        ),
        Descriptor::choice(
            DecCphyRate,
            "Symbol rate",
            |u| crate::decode::cphy_rate_label(u.m.dec_cphy_rate),
            |_| crate::decode::CPHY_RATES.into_iter().map(crate::decode::cphy_rate_label).collect(),
        ),
        Descriptor::choice(
            DecFmt,
            "Format",
            |u| if u.m.dec_ascii { "ASCII" } else { "Hex" }.into(),
            |_| ["Hex", "ASCII"].into_iter().map(String::from).collect(),
        ),
        option::<0>(),
        option::<1>(),
        option::<2>(),
        option::<3>(),
        Descriptor::value(DecThr, "Threshold", |u| si(u.m.dec_thr as f64, "V")).when(|u| !u.m.dec_on_la()),
        Descriptor::action(DecTable, "Event table"),
        Descriptor::action(DecExport, "Export events"),
    ]
};

const fn option<const K: usize>() -> Descriptor {
    Descriptor::choice(
        Item::DecOpt(K),
        "Option",
        |u| {
            u.m.dec_proto
                .module()
                .and_then(|p| p.options().get(K))
                .map_or(String::new(), |o| o.choices[u.m.dec_opts()[K]].into())
        },
        |u| {
            u.m.dec_proto
                .module()
                .and_then(|p| p.options().get(K))
                .map_or(Vec::new(), |o| o.choices.iter().map(|c| c.to_string()).collect())
        },
    )
    .when(|u| u.m.dec_proto.module().is_some_and(|p| K < p.options().len()))
}

fn source_value(ui: &Ui, role: usize, analog: Option<usize>) -> String {
    if ui.m.dec_on_la() {
        ui.m.dec_la[role].map_or("None".into(), |n| la::NAMES[n as usize].into())
    } else {
        analog.map_or("None".into(), decode_source_name)
    }
}

fn analog_sources(ui: &Ui, optional: bool) -> Vec<String> {
    optional
        .then(|| "None".into())
        .into_iter()
        .chain(ui.decoder_sources().into_iter().map(decode_source_name))
        .collect()
}

fn source_options(ui: &Ui, optional: bool) -> Vec<String> {
    if ui.m.dec_on_la() {
        optional.then(|| "None".into()).into_iter().chain(la::NAMES.into_iter().map(String::from)).collect()
    } else {
        analog_sources(ui, optional)
    }
}

fn ordered_rows(ui: &Ui, _: MenuId) -> Vec<&'static Descriptor> {
    use crate::protocols::Row;
    use Item::*;
    let shared = ui.m.dec_proto.module().map_or(&[][..], |p| p.shared()).iter().map(|r| match r {
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
    [DecSel, DecOn, DecProto, DecIn, DecSrc, DecSrc2, DecCs, DecSrc4, DecDiff]
        .into_iter()
        .chain(shared)
        .chain((0..crate::protocols::OPTS).map(DecOpt))
        .chain([DecThr, DecTable, DecExport])
        .map(|it| super::descriptor(it).expect("every decoder row has a registered descriptor"))
        .collect()
}

#[cfg(test)]
pub(in crate::ui) fn decode_items(ui: &Ui) -> Vec<Item> {
    super::items(ui, MenuId::Decode)
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
