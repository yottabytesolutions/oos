// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
#[cfg_attr(not(feature = "decoders"), ignore = "needs every decoder")]
fn uart_from_the_decoder_settings() {
    use oos_hal::dho::uarttrig::{Cond, ErrKind, Parity, Relation};
    let mut m = Model::default();
    (m.trig_type, m.dec_proto, m.dec_src, m.dec_bits, m.dec_baud) = (TrigType::Bus, Proto::UART, 1, 8, 7);
    // (condition, value) -> receiver condition (None = the software trigger)
    let cases = [
        (BusWhat::Frame, 0, Some(Cond::Start)),
        (BusWhat::Data, 0x41, Some(Cond::Data(Relation::Eq, 0x41, 0x41))),
        (BusWhat::Data, 0x141, None),
        (BusWhat::Error, 0, Some(Cond::Error(ErrKind::Either))),
        (BusWhat::Id, 0x41, None),
    ];
    for (what, v, want) in cases {
        (m.trig_bus, m.trig_bus_val) = (what, v);
        assert_eq!(uart_of(&m).map(|u| u.cond), want, "{:?} {:#x}", what, v);
    }
    m.trig_bus = BusWhat::Frame;
    m.dec_parity = oos_dsp::decode::Parity::Even;
    let u = uart_of(&m).unwrap();
    assert_eq!((u.parity, u.baud, u.bits, u.invert), (Parity::Even, crate::decode::BAUDS[7], 8, false));
    // 7 data bits: a value with bit 7 set cannot match
    (m.dec_bits, m.trig_bus, m.trig_bus_val) = (7, BusWhat::Data, 0x80);
    assert_eq!(uart_of(&m), None);
    // other protocols, the LA as input, RS-485: the software trigger
    (m.dec_bits, m.trig_bus) = (8, BusWhat::Frame);
    for f in [
        |m: &mut Model| m.dec_proto = Proto::SPI,
        |m: &mut Model| m.dec_proto = Proto::from_key("Uart9").unwrap(),
        |m: &mut Model| m.dec_diff = Some(2),
        |m: &mut Model| m.trig_type = TrigType::Edge,
    ] {
        let mut x = m.clone();
        f(&mut x);
        assert_eq!(uart_of(&x), None);
    }
    assert!(!serial_hw_from("") && serial_hw_from("trig.serial_hw = on\n"));
    assert!(uart_debug(&m, false).starts_with("off,src=CH2,"), "{}", uart_debug(&m, false));
}

#[test]
fn spi_and_i2c_from_the_decoder_settings() {
    use oos_hal::dho::serialtrig::I2cCond;
    let mut m = Model::default();
    (m.trig_type, m.dec_proto, m.dec_src, m.dec_src2, m.dec_cs, m.dec_bits, m.dec_mode) =
        (TrigType::Bus, Proto::SPI, 0, 1, Some(2), 8, 0);
    // (condition, value) -> (value, mask) of the SPI compare; None = software
    let cases = [
        (BusWhat::Frame, 0, Some((0, 0))),
        (BusWhat::Data, 0x5a, Some((0x5a, 0xff))),
        (BusWhat::Data, 0x15a, None),
        (BusWhat::Error, 0, None),
        (BusWhat::Id, 1, None),
    ];
    for (what, v, want) in cases {
        (m.trig_bus, m.trig_bus_val) = (what, v);
        assert_eq!(spi_of(&m).map(|s| (s.min, s.mask)), want, "{:?}", what);
    }
    (m.trig_bus, m.trig_bus_val) = (BusWhat::Data, 0x5a);
    let s = spi_of(&m).unwrap();
    assert_eq!((s.clk, s.data, s.cs, s.clk_falling, s.width), (2, 1, 3, false, 8));
    m.dec_mode = 1;
    assert!(spi_of(&m).unwrap().clk_falling);
    m.dec_cs = None;
    assert_eq!(spi_of(&m), None, "no chip select: software");
    // I2C: SDA on dec_src, SCL on dec_src2
    let mut m = Model::default();
    (m.trig_type, m.dec_proto, m.dec_src, m.dec_src2) = (TrigType::Bus, Proto::I2C, 0, 1);
    let cases = [
        (BusWhat::Frame, 0, Some(I2cCond::Start)),
        (BusWhat::Id, 0x50, Some(I2cCond::Address)),
        (BusWhat::Id, 0x150, None),
        (BusWhat::Error, 0, Some(I2cCond::MissingAck)),
        (BusWhat::Data, 0x12, None),
    ];
    for (what, v, want) in cases {
        (m.trig_bus, m.trig_bus_val) = (what, v);
        assert_eq!(i2c_of(&m).map(|i| i.cond), want, "{:?}", what);
    }
    // data in the first two bytes after the address: the receiver, big-endian, mask of two bytes
    (m.trig_bus, m.trig_bus_val, m.ser.bytes) = (BusWhat::Data, 0x1234, 2);
    let d = i2c_of(&m).unwrap();
    assert_eq!((d.cond, d.bytes, d.data), (I2cCond::Data, 2, (0x1234, 0x1234, 0xffff)));
    m.ser.bytes = 0;
    (m.trig_bus, m.trig_bus_val) = (BusWhat::Id, 0x50);
    let i = i2c_of(&m).unwrap();
    assert_eq!((i.scl, i.sda, i.addr), (2, 1, (0x50, 0x50, 0x7f)));
    assert!(
        serial_debug(&m, false).starts_with("off,proto=I2C,scl=CH2,sda=CH1,cond=Address,r2190="),
        "{}",
        serial_debug(&m, false)
    );
    // lines that are not acquired fall back to the edge trigger
    assert!(serial_lines_acquired(&Trig::I2c(i), 0b11) && !serial_lines_acquired(&Trig::I2c(i), 0b01));
    assert!(serial_lines_acquired(&Trig::Edge, 0));
}

#[test]
fn serial_settings_reach_the_receivers() {
    use oos_hal::dho::serialtrig::{Dir, I2cCond};
    use oos_hal::dho::uarttrig::{Cond, ErrKind, Relation, Stop};
    // UART: relation and limit, polarity, stop bits, error kind; a mask keeps the software
    let mut m = Model::default();
    (m.trig_type, m.dec_proto, m.dec_bits, m.trig_bus, m.trig_bus_val) =
        (TrigType::Bus, Proto::UART, 8, BusWhat::Data, 0x30);
    (m.ser.rel, m.ser.max, m.ser.uart_inv, m.ser.uart_stop) = (Rel::In, 0x39, true, StopBits::Two);
    let u = uart_of(&m).unwrap();
    assert_eq!((u.cond, u.invert, u.stop), (Cond::Data(Relation::In, 0x30, 0x39), true, Stop::Two));
    m.ser.mask = 0x0f;
    assert_eq!(uart_of(&m), None);
    (m.ser.mask, m.ser.max) = (u64::MAX, 0x139);
    assert_eq!(uart_of(&m), None, "a limit wider than the data bits");
    (m.trig_bus, m.ser.uart_err) = (BusWhat::Error, UartErr::Parity);
    assert_eq!(uart_of(&m).unwrap().cond, Cond::Error(ErrKind::Parity));
    // SPI: without CS the timeout frames; relation, limit and mask to the width; CS polarity
    let mut m = Model::default();
    (m.trig_type, m.dec_proto, m.dec_src, m.dec_src2, m.dec_cs, m.dec_bits) =
        (TrigType::Bus, Proto::SPI, 0, 1, None, 16);
    (m.trig_bus, m.trig_bus_val, m.ser.rel, m.ser.max, m.ser.mask) = (BusWhat::Data, 0x1000, Rel::Out, 0x2000, 0xff00);
    assert_eq!(spi_of(&m), None, "no CS and no timeout");
    m.ser.spi_timeout = 2e-6;
    let s = spi_of(&m).unwrap();
    assert_eq!(
        (s.timeout, s.cs, s.rel, s.min, s.max, s.mask),
        (Some(2e-6f32 as f64), 2, Relation::Out, 0x1000, 0x2000, 0xff00)
    );
    (m.dec_cs, m.ser.spi_cs_high) = (Some(2), true);
    let s = spi_of(&m).unwrap();
    assert_eq!((s.timeout, s.cs, s.cs_high), (None, 3, true));
    // I2C: 10-bit address range, read only
    let mut m = Model::default();
    (m.trig_type, m.dec_proto, m.dec_src, m.dec_src2, m.trig_bus, m.trig_bus_val) =
        (TrigType::Bus, Proto::I2C, 0, 1, BusWhat::Id, 0x200);
    (m.ser.ten_bit, m.ser.addr_rel, m.ser.addr_max, m.ser.dir) = (true, Rel::In, 0x2ff, I2cDir::Read);
    let i = i2c_of(&m).unwrap();
    assert_eq!(
        (i.cond, i.ten_bit, i.addr_rel, i.addr, i.dir),
        (I2cCond::Address, true, Relation::In, (0x200, 0x2ff, 0x3ff), Dir::Read)
    );
    m.ser.ten_bit = false;
    assert_eq!(i2c_of(&m), None, "a 10-bit value with 7-bit addresses");
}

#[test]
fn uart_shifts_the_record_to_the_software_point() {
    let mut m = Model::default();
    (m.trig_type, m.dec_proto, m.dec_src, m.trig_bus) = (TrigType::Bus, Proto::UART, 0, BusWhat::Frame);
    let u = uart_of(&m).unwrap();
    let mut s = settings(&m, false, &Default::default());
    let d0 = s.delay;
    s.trigger = Trig::Uart(u);
    // `settings` applies the shift only with the switch on; the same arithmetic here
    if let (Trig::Uart(u), Slow::Off) = (&s.trigger, slow_mode(&m)) {
        s.delay += uarttrig::sw_offset_s(u);
    }
    let bit = 1.0 / crate::decode::BAUDS[m.dec_baud];
    assert!((s.delay - d0 - (9.4 * bit - 4.0 / 156.25e6)).abs() < 1e-12);
}
