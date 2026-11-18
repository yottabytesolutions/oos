// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn every_decoder_row_keeps_its_static_reads() {
    use Item::*;
    use Kind::*;
    let u = configured_decoder();
    let sources = ch_names();
    let optional = std::iter::once("None".into()).chain(sources.clone()).collect::<Vec<_>>();
    for (it, kind, label, value, options) in [
        (Open(MenuId::Decode), Action, "Decode", "".into(), vec![]),
        (DecSel, Choice, "Decoder no.", "D3".into(), strings(&["D1", "D2", "D3", "D4"])),
        (DecOn, Choice, "Decoder", "On".into(), strings(&["Off", "On"])),
        (DecProto, Choice, "Protocol", "UART".into(), PROTOS.iter().map(|p| p.name().into()).collect()),
        (DecSrc, Choice, "Source", "CH2".into(), sources.clone()),
        (DecSrc2, Choice, "Clock", "CH1".into(), sources),
        (DecCs, Choice, "CS", "None".into(), optional.clone()),
        (DecSrc4, Choice, "Source 4", "None".into(), optional.clone()),
        (DecDiff, Choice, "B line (A-B)", "None".into(), optional),
        (
            DecBaud,
            Choice,
            "Baud",
            crate::decode::BAUDS[u.m.dec_baud].to_string(),
            crate::decode::BAUDS.iter().map(ToString::to_string).collect(),
        ),
        (
            DecCphyRate,
            Choice,
            "Symbol rate",
            crate::decode::cphy_rate_label(u.m.dec_cphy_rate),
            crate::decode::CPHY_RATES.into_iter().map(crate::decode::cphy_rate_label).collect(),
        ),
        (
            DecCanRate,
            Choice,
            "Bit rate",
            si(crate::decode::CAN_RATES[u.m.dec_canrate], "bit/s"),
            crate::decode::CAN_RATES.iter().map(|&b| si(b, "bit/s")).collect(),
        ),
        (DecCanH, Choice, "Signal", "CAN_H".into(), strings(&["RX/TX", "CAN_H"])),
        (
            DecUsbSpeed,
            Choice,
            "Speed",
            USPEED_NAMES[u.m.dec_uspeed].into(),
            USPEED_NAMES.into_iter().map(String::from).collect(),
        ),
        (DecMode, Choice, "Mode", "3".into(), strings(&["0", "1", "2", "3"])),
        (DecBits, Value, "Data bits", "9".into(), vec![]),
        (DecParity, Choice, "Parity", "Odd".into(), strings(&["None", "Even", "Odd"])),
        (DecThr, Value, "Threshold", "-500 mV".into(), vec![]),
        (DecFmt, Choice, "Format", "ASCII".into(), strings(&["Hex", "ASCII"])),
        (DecTable, Action, "Event table", "".into(), vec![]),
        (DecExport, Action, "Export events", "".into(), vec![]),
    ] {
        check(&u, it, kind, label, &value, options);
    }
}

#[test]
fn each_protocol_option_keeps_its_choices_value_and_committed_identity() {
    let mut u = demo();
    for &proto in PROTOS.iter() {
        u.m.dec_proto = proto;
        let module = proto.module().unwrap();
        for k in 0..crate::protocols::OPTS {
            let item = Item::DecOpt(k);
            assert_eq!(Ui::label(item), "Option");
            assert_eq!(Ui::kind(item), Kind::Choice);
            if let Some(option) = module.options().get(k) {
                assert_eq!(u.options(item), option.choices);
                assert_eq!(u.item_label(item), option.name);
                for (choice, name) in option.choices.iter().enumerate() {
                    u.m.dec_opt[k] = choice as u8;
                    assert_eq!(u.value(item), *name, "{proto:?} {k}");
                    assert_eq!(active_choice::index(&u, item), Some(choice));
                }
            } else {
                assert_eq!(u.value(item), "");
                assert!(u.options(item).is_empty());
            }
        }
    }
}

fn expected_rows(u: &Ui) -> Vec<Item> {
    use crate::protocols::Row;
    use Item::*;
    let p = u.m.dec_proto;
    let mut rows = vec![DecSel, DecOn, DecProto];
    if u.m.la_on() && p != Proto::CPHY {
        rows.push(DecIn);
    }
    rows.push(DecSrc);
    for (role, item) in [(1, DecSrc2), (2, DecCs), (3, DecSrc4)] {
        if p.roles()[role].is_some() {
            rows.push(item);
        }
    }
    if p.diff_ok() && !u.m.dec_on_la() {
        rows.push(DecDiff);
    }
    rows.extend(p.module().unwrap().shared().iter().map(|row| match row {
        Row::Baud => DecBaud,
        Row::Bits => DecBits,
        Row::Parity => DecParity,
        Row::SpiMode => DecMode,
        Row::CanRate => DecCanRate,
        Row::CanH => DecCanH,
        Row::UsbSpeed => DecUsbSpeed,
        Row::CphyRate => DecCphyRate,
        Row::Fmt => DecFmt,
    }));
    rows.extend((0..p.module().unwrap().options().len()).map(DecOpt));
    if !u.m.dec_on_la() {
        rows.push(DecThr);
    }
    rows.extend([DecTable, DecExport]);
    rows
}

#[test]
fn decoder_visibility_preserves_each_protocols_role_and_option_order() {
    let mut u = demo();
    for &proto in PROTOS.iter() {
        u.m.dec_proto = proto;
        for la in [false, true] {
            u.m.la.on = la;
            for input in [false, true] {
                u.m.dec_la_in = input;
                assert_eq!(u.menu_items(MenuId::Decode), expected_rows(&u), "{proto:?} {la} {input}");
                assert_eq!(u.decode_items(), expected_rows(&u));
            }
        }
    }
}

#[test]
fn decoder_sources_preserve_analog_references_and_optional_logic_roles() {
    let mut u = demo();
    u.refs[3] = Some(wave());
    u.m.dec_src = DEC_REF_BASE + 3;
    u.m.dec_src2 = DEC_REF_BASE + 3;
    u.m.dec_cs = Some(DEC_REF_BASE + 3);
    u.m.dec_src4 = Some(DEC_REF_BASE + 3);
    u.m.dec_diff = Some(DEC_REF_BASE + 3);
    for &proto in PROTOS.iter() {
        u.m.dec_proto = proto;
        for la in [false, true] {
            u.m.la.on = la;
            u.m.dec_la_in = true;
            u.m.dec_la = [Some(15), Some(0), None, Some(7)];
            for (it, role) in [(Item::DecSrc, 0), (Item::DecSrc2, 1), (Item::DecCs, 2), (Item::DecSrc4, 3)] {
                let optional = role >= 2 && (role == 3 || proto != Proto::CPHY);
                let mut want = if optional { vec!["None".into()] } else { vec![] };
                if u.m.dec_on_la() {
                    want.extend((0..16).map(|n| format!("D{n}")));
                    assert_eq!(u.value(it), ["D15", "D0", "None", "D7"][role]);
                } else {
                    want.extend(ch_names());
                    want.push("R4".into());
                    assert_eq!(u.value(it), "R4");
                }
                assert_eq!(u.options(it), want, "{proto:?} {it:?}");
            }
            assert_eq!(u.value(Item::DecDiff), "R4");
            assert_eq!(
                u.options(Item::DecDiff),
                std::iter::once("None".into()).chain(ch_names()).chain(["R4".into()]).collect::<Vec<_>>()
            );
        }
    }
}

fn configured_decoder() -> Ui {
    let mut u = demo();
    u.m.dec_sel = 2;
    u.m.dec_on = true;
    u.m.dec_proto = Proto::UART;
    u.m.dec_src = 1;
    u.m.dec_src2 = 0;
    u.m.dec_cs = None;
    u.m.dec_src4 = None;
    u.m.dec_diff = None;
    u.m.dec_mode = 3;
    u.m.dec_bits = 9;
    u.m.dec_thr = -0.5;
    u.m.dec_ascii = true;
    u.m.dec_canh = true;
    u.m.dec_parity = oos_dsp::decode::Parity::Odd;
    u
}
