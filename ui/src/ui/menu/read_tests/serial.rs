// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn every_serial_row_keeps_its_kind_label_value_and_options() {
    use Item::*;
    use Kind::*;
    let mut u = demo();
    u.m.dec_proto = Proto::UART;
    u.m.ser.rel = Rel::In;
    u.m.ser.max = 0x42;
    u.m.ser.mask = 0xff;
    u.m.ser.uart_inv = true;
    u.m.ser.uart_stop = StopBits::OneHalf;
    u.m.ser.uart_err = crate::model::UartErr::Framing;
    u.m.ser.spi_cs_high = true;
    u.m.ser.spi_timeout = 2e-6;
    u.m.ser.ten_bit = true;
    u.m.ser.bytes = 3;
    let keypad = KEYPAD.iter().map(|k| k.to_string()).collect::<Vec<_>>();
    for (it, kind, label, value, options) in [
        (SerRel, Choice, "Relation", "In range", RELS.iter().map(|r| r.name().into()).collect()),
        (SerMax, Choice, "Max", "0x42", keypad.clone()),
        (SerMask, Choice, "Mask", "0xFF", keypad),
        (UartPol, Choice, "Polarity", "Inverted", strings(&["Normal", "Inverted"])),
        (UartStop, Choice, "Stop bits", "1.5", strings(&["1", "1.5", "2"])),
        (UartErr, Choice, "Error type", "Framing", strings(&["Any", "Parity", "Framing"])),
        (SpiCs, Choice, "CS", "Active high", strings(&["Active low", "Active high"])),
        (SpiTimeout, Value, "Timeout", "2 \u{b5}s", vec![]),
        (I2cDir, Choice, "Direction", "Either", strings(&["Either", "Write", "Read"])),
        (I2cAddrBits, Choice, "Address", "10-bit", strings(&["7-bit", "10-bit"])),
        (I2cBytes, Choice, "Bytes", "3", strings(&["Any", "1", "2", "3", "4", "5"])),
    ] {
        check(&u, it, kind, label, value, options);
    }
    u.m.ser.spi_timeout = 0.0;
    u.m.ser.bytes = 0;
    assert_eq!(u.value(SpiTimeout), "Off");
    assert_eq!(u.value(I2cBytes), "Any");
    for it in [SerRel, UartPol, UartStop, UartErr, SpiCs, I2cDir, I2cAddrBits, I2cBytes] {
        for name in u.options(it) {
            u.select_option(it, &name);
            assert_eq!(u.value(it), name, "{it:?}");
        }
    }
}

fn expected_rows(u: &Ui) -> Vec<Item> {
    use Item::*;
    let (m, s) = (&u.m, &u.m.ser);
    let relation = |rel: Rel, mask: bool| {
        let mut rows = vec![SerRel];
        if rel.uses_max() {
            rows.push(SerMax);
        }
        if mask {
            rows.push(SerMask);
        }
        rows
    };
    match (m.dec_proto, m.trig_bus) {
        (Proto::UART, what) => {
            let mut rows = if what == BusWhat::Data { relation(s.rel, false) } else { vec![] };
            if what == BusWhat::Error {
                rows.push(UartErr);
            }
            rows.extend([UartPol, UartStop]);
            rows
        }
        (Proto::SPI, what) => {
            let mut rows = if what == BusWhat::Data { relation(s.rel, true) } else { vec![] };
            rows.push(if m.dec_cs.is_some() { SpiCs } else { SpiTimeout });
            rows
        }
        (Proto::I2C, BusWhat::Id) => relation(s.addr_rel, false).into_iter().chain([I2cAddrBits, I2cDir]).collect(),
        (Proto::I2C, BusWhat::Data) => relation(s.rel, true).into_iter().chain([I2cBytes, I2cDir]).collect(),
        _ => vec![],
    }
}

#[test]
fn serial_visibility_follows_each_protocol_condition_relation_and_chip_select() {
    let mut u = demo();
    u.m.trig_type = TrigType::Bus;
    for &p in PROTOS.iter() {
        u.m.dec_proto = p;
        for what in BUS_WHATS {
            u.m.trig_bus = what;
            for relation in RELS {
                u.m.ser.rel = relation;
                for address_relation in RELS {
                    u.m.ser.addr_rel = address_relation;
                    for cs in [None, Some(1)] {
                        u.m.dec_cs = cs;
                        assert_eq!(u.serial_items(), expected_rows(&u), "{p:?} {what:?} {relation:?}");
                    }
                }
            }
        }
    }
}
