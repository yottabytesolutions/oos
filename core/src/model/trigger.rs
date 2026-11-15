// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slope {
    Rise,
    Fall,
    Either,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrigMode {
    Auto,
    Normal,
    Single,
}

pub use oos_dsp::measure::{Meas, ALL as ALL_MEAS};
pub use oos_dsp::sim::VideoStd;
pub use oos_dsp::trigger::{Bit, Pol};

/// Trigger types (the software trigger engine of oos-dsp; on the unit later the TPU, docs/protocols/trigger.md).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrigType {
    Edge,
    Pulse,
    Slope,
    Video,
    Pattern,
    Duration,
    Timeout,
    Runt,
    Window,
    Delay,
    SetupHold,
    NthEdge,
    /// protocol trigger: the decoder of the Decode menu finds the event (BusWhat, value)
    Bus,
}

pub const TRIG_TYPES: [TrigType; 13] = [
    TrigType::Edge,
    TrigType::Pulse,
    TrigType::Slope,
    TrigType::Video,
    TrigType::Pattern,
    TrigType::Duration,
    TrigType::Timeout,
    TrigType::Runt,
    TrigType::Window,
    TrigType::Delay,
    TrigType::SetupHold,
    TrigType::NthEdge,
    TrigType::Bus,
];

/// What the protocol trigger looks for in the decoded packets (`trig::find_all`); the trigger point is the end of
/// the matching field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BusWhat {
    /// every frame start (SOF, break, I2C start, 1553 command word, UART / SPI word)
    Frame,
    /// identifier / address / 1553 command word equal to the value
    Id,
    /// data byte / word / audio sample equal to the value
    Data,
    /// any packet with a failed check (CRC, parity, checksum, NACK, missing ACK)
    Error,
}
pub const BUS_WHATS: [BusWhat; 4] = [BusWhat::Frame, BusWhat::Id, BusWhat::Data, BusWhat::Error];

/// How a protocol value compares (`SerialTrig`): with the value (`trig_bus_val`) as min, `max` as the upper limit, all
/// after the mask. The relations of the TPU's serial receivers (docs/fork/serial-ui.md).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rel {
    Eq,
    Ne,
    /// greater than the value
    Gt,
    /// less than max
    Lt,
    /// value <= x <= max
    In,
    /// x < value or x > max
    Out,
}
pub const RELS: [Rel; 6] = [Rel::Eq, Rel::Ne, Rel::Gt, Rel::Lt, Rel::In, Rel::Out];
impl Rel {
    pub fn name(self) -> &'static str {
        match self {
            Rel::Eq => "=",
            Rel::Ne => "\u{2260}",
            Rel::Gt => ">",
            Rel::Lt => "<",
            Rel::In => "In range",
            Rel::Out => "Out of range",
        }
    }

    /// The relation needs the upper limit.
    pub fn uses_max(self) -> bool {
        matches!(self, Rel::Lt | Rel::In | Rel::Out)
    }

    /// `x` against `min`/`max` after `mask`.
    pub fn test(self, x: u64, min: u64, max: u64, mask: u64) -> bool {
        let (x, a, b) = (x & mask, min & mask, max & mask);
        match self {
            Rel::Eq => x == a,
            Rel::Ne => x != a,
            Rel::Gt => x > a,
            Rel::Lt => x < b,
            Rel::In => a <= x && x <= b,
            Rel::Out => x < a || x > b,
        }
    }
}

/// UART stop bits of the trigger.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StopBits {
    One,
    OneHalf,
    Two,
}
pub const STOP_BITS: [StopBits; 3] = [StopBits::One, StopBits::OneHalf, StopBits::Two];

/// Which UART error the Error condition looks for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UartErr {
    Either,
    Parity,
    Framing,
}
pub const UART_ERRS: [UartErr; 3] = [UartErr::Either, UartErr::Parity, UartErr::Framing];

/// I2C transfer direction of the address condition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum I2cDir {
    Either,
    Write,
    Read,
}
pub const I2C_DIRS: [I2cDir; 3] = [I2cDir::Either, I2cDir::Write, I2cDir::Read];

/// The protocol trigger's settings beyond condition and value (Trigger > Protocol, `:TRIGger:RS232|SPI|IIC:*`); the
/// TPU's receivers take them all (`trig.serial_hw = on`), the software trigger as far as the decoders show it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SerialTrig {
    pub rel: Rel,
    /// upper limit of `rel` (Lt, In, Out)
    pub max: u64,
    /// bits of the value that count (SPI, I2C data)
    pub mask: u64,
    /// UART: idle low (the line inverted)
    pub uart_inv: bool,
    pub uart_stop: StopBits,
    pub uart_err: UartErr,
    /// SPI: chip select active high
    pub spi_cs_high: bool,
    /// SPI without chip select: a clock pause longer than this starts a frame, s (0 = the decoder's default)
    pub spi_timeout: f32,
    /// I2C address condition: relation and upper limit, direction, 10-bit addresses
    pub addr_rel: Rel,
    pub addr_max: u32,
    pub dir: I2cDir,
    pub ten_bit: bool,
    /// I2C data condition: 0 = any data byte of a frame (the software trigger), 1..5 = the first `bytes` data bytes
    /// after the address, the first the most significant (also the TPU's receiver)
    pub bytes: u8,
}

impl Default for SerialTrig {
    fn default() -> SerialTrig {
        SerialTrig {
            rel: Rel::Eq,
            max: 0xff,
            mask: u64::MAX,
            uart_inv: false,
            uart_stop: StopBits::One,
            uart_err: UartErr::Either,
            spi_cs_high: false,
            spi_timeout: 0.0,
            addr_rel: Rel::Eq,
            addr_max: 0x7f,
            dir: I2cDir::Either,
            ten_bit: false,
            bytes: 0,
        }
    }
}
impl BusWhat {
    pub fn name(self) -> &'static str {
        match self {
            BusWhat::Frame => "Frame",
            BusWhat::Id => "ID/Addr",
            BusWhat::Data => "Data",
            BusWhat::Error => "Error",
        }
    }
}

impl TrigType {
    pub fn name(self) -> &'static str {
        match self {
            TrigType::Edge => "Edge",
            TrigType::Pulse => "Pulse",
            TrigType::Slope => "Slope",
            TrigType::Video => "Video",
            TrigType::Pattern => "Pattern",
            TrigType::Duration => "Duration",
            TrigType::Timeout => "Timeout",
            TrigType::Runt => "Runt",
            TrigType::Window => "Window",
            TrigType::Delay => "Delay",
            TrigType::SetupHold => "Setup/Hold",
            TrigType::NthEdge => "Nth edge",
            TrigType::Bus => "Bus",
        }
    }
}

/// Time condition kind of the "When" setting (Off only for runt).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WhenKind {
    Off,
    Gt,
    Lt,
    In,
    Out,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WinMode {
    Enter,
    Exit,
    Inside,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VSync {
    AllLines,
    Line,
    Odd,
    Even,
    AllFields,
}

/// 1-2-5 ladder for trigger times: 1 ns .. 10 s.
pub fn time_ladder() -> Vec<f32> {
    let mut v = Vec::new();
    let mut d = 1e-9_f64;
    while d < 20.0 {
        for m in [1.0, 2.0, 5.0] {
            if d * m <= 10.0 {
                v.push((d * m) as f32);
            }
        }
        d *= 10.0;
    }
    v
}
