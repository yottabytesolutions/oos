// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

pub(super) const AREA: Area = Area { kind, label, value, adjust, options, ..Area::EMPTY };

fn kind(it: Item) -> Option<Kind> {
    Some(match it {
        Item::SpiTimeout => Kind::Value,
        _ => return None,
    })
}

fn label(it: Item) -> Option<&'static str> {
    Some(match it {
        Item::SerRel => "Relation",
        Item::SerMax => "Max",
        Item::SerMask => "Mask",
        Item::UartPol => "Polarity",
        Item::UartStop => "Stop bits",
        Item::UartErr => "Error type",
        Item::SpiCs => "CS",
        Item::SpiTimeout => "Timeout",
        Item::I2cDir => "Direction",
        Item::I2cAddrBits => "Address",
        Item::I2cBytes => "Bytes",
        _ => return None,
    })
}

fn value(ui: &Ui, it: Item) -> Option<String> {
    Some(match it {
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
        | Item::I2cBytes => ui.serial_value(it),
        _ => return None,
    })
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

fn options(_ui: &Ui, it: Item) -> Option<Vec<String>> {
    let v: &[&str] = match it {
        Item::SerMax | Item::SerMask => return Some(KEYPAD.iter().map(|k| k.to_string()).collect()),
        Item::SerRel
        | Item::UartPol
        | Item::UartStop
        | Item::UartErr
        | Item::SpiCs
        | Item::I2cDir
        | Item::I2cAddrBits
        | Item::I2cBytes => return Some(Ui::serial_options(it)),
        _ => &[],
    };
    Some(v.iter().map(|s| s.to_string()).collect())
}
