// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;
use crate::model::UartErr as ErrorKind;

pub(super) const AREA: Area = Area { descriptors: ROWS, adjust, ..Area::EMPTY };

const ROWS: &[Descriptor] = {
    use Item::*;
    &[
        Descriptor::choice(
            SerRel,
            "Relation",
            |u| relation(u).name().into(),
            |_| RELS.iter().map(|r| r.name().to_string()).collect(),
        )
        .when(has_relation),
        Descriptor::choice(SerMax, "Max", |u| format!("0x{:X}", u.hex_value(SerMax)), keypad)
            .when(|u| has_relation(u) && relation(u).uses_max()),
        Descriptor::choice(SerMask, "Mask", |u| format!("0x{:X}", u.hex_value(SerMask)), keypad)
            .when(|u| u.m.trig_bus == BusWhat::Data && matches!(u.m.dec_proto, Proto::SPI | Proto::I2C)),
        Descriptor::choice(
            UartErr,
            "Error type",
            |u| {
                match u.m.ser.uart_err {
                    ErrorKind::Either => "Any",
                    ErrorKind::Parity => "Parity",
                    ErrorKind::Framing => "Framing",
                }
                .into()
            },
            |_| ["Any", "Parity", "Framing"].into_iter().map(String::from).collect(),
        )
        .when(|u| u.m.dec_proto == Proto::UART && u.m.trig_bus == BusWhat::Error),
        Descriptor::choice(
            UartPol,
            "Polarity",
            |u| if u.m.ser.uart_inv { "Inverted" } else { "Normal" }.into(),
            |_| ["Normal", "Inverted"].into_iter().map(String::from).collect(),
        )
        .when(|u| u.m.dec_proto == Proto::UART),
        Descriptor::choice(
            UartStop,
            "Stop bits",
            |u| {
                match u.m.ser.uart_stop {
                    StopBits::One => "1",
                    StopBits::OneHalf => "1.5",
                    StopBits::Two => "2",
                }
                .into()
            },
            |_| ["1", "1.5", "2"].into_iter().map(String::from).collect(),
        )
        .when(|u| u.m.dec_proto == Proto::UART),
        Descriptor::choice(
            SpiCs,
            "CS",
            |u| if u.m.ser.spi_cs_high { "Active high" } else { "Active low" }.into(),
            |_| ["Active low", "Active high"].into_iter().map(String::from).collect(),
        )
        .when(|u| u.m.dec_proto == Proto::SPI && u.m.dec_cs.is_some()),
        Descriptor::value(SpiTimeout, "Timeout", |u| {
            if u.m.ser.spi_timeout > 0.0 {
                si_short(u.m.ser.spi_timeout as f64, "s")
            } else {
                "Off".into()
            }
        })
        .when(|u| u.m.dec_proto == Proto::SPI && u.m.dec_cs.is_none()),
        Descriptor::choice(
            I2cAddrBits,
            "Address",
            |u| if u.m.ser.ten_bit { "10-bit" } else { "7-bit" }.into(),
            |_| ["7-bit", "10-bit"].into_iter().map(String::from).collect(),
        )
        .when(|u| u.m.dec_proto == Proto::I2C && u.m.trig_bus == BusWhat::Id),
        Descriptor::choice(
            I2cBytes,
            "Bytes",
            |u| if u.m.ser.bytes == 0 { "Any".into() } else { u.m.ser.bytes.to_string() },
            |_| ["Any", "1", "2", "3", "4", "5"].into_iter().map(String::from).collect(),
        )
        .when(|u| u.m.dec_proto == Proto::I2C && u.m.trig_bus == BusWhat::Data),
        Descriptor::choice(
            I2cDir,
            "Direction",
            |u| format!("{:?}", u.m.ser.dir),
            |_| ["Either", "Write", "Read"].into_iter().map(String::from).collect(),
        )
        .when(|u| u.m.dec_proto == Proto::I2C && matches!(u.m.trig_bus, BusWhat::Id | BusWhat::Data)),
    ]
};

fn keypad(_: &Ui) -> Vec<String> {
    KEYPAD.iter().map(|k| k.to_string()).collect()
}

fn relation(ui: &Ui) -> Rel {
    if ui.m.dec_proto == Proto::I2C && ui.m.trig_bus == BusWhat::Id {
        ui.m.ser.addr_rel
    } else {
        ui.m.ser.rel
    }
}

fn has_relation(ui: &Ui) -> bool {
    matches!(
        (ui.m.dec_proto, ui.m.trig_bus),
        (Proto::UART | Proto::SPI, BusWhat::Data) | (Proto::I2C, BusWhat::Id | BusWhat::Data)
    )
}

pub(in crate::ui) fn serial_items(ui: &Ui) -> Vec<Item> {
    ROWS.iter().filter(|row| (row.visible)(ui)).map(|row| row.item).collect()
}

fn adjust(ui: &mut Ui, it: Item, d: i32) -> bool {
    match it {
        Item::SerRel
        | Item::SerMax
        | Item::SerMask
        | Item::UartPol
        | Item::UartStop
        | Item::UartErr
        | Item::SpiCs
        | Item::SpiTimeout
        | Item::I2cDir
        | Item::I2cAddrBits
        | Item::I2cBytes => ui.serial_adjust(it, d),
        _ => {}
    }
    false
}
