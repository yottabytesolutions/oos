// SPDX-License-Identifier: GPL-2.0-or-later

//! Protocol settings metadata and evaluated values, independent of decoder implementations.

pub const OPTS: usize = 4;

/// A menu option supplied by the registry, without decoder callbacks.
#[derive(Clone, Copy, Debug)]
pub struct Opt {
    pub name: &'static str,
    pub choices: &'static [&'static str],
    pub default: usize,
}

#[derive(Clone, Copy, Debug)]
pub struct ModuleInfo {
    pub key: &'static str,
    pub name: &'static str,
    pub roles: [Option<&'static str>; 4],
    pub diff_ok: bool,
    pub options: &'static [Opt],
}

/// Results evaluated by the adapter for the selected module and its fitted options.
#[derive(Clone, Copy, Debug)]
pub struct ModuleValues {
    pub id: u8,
    pub bit_time: f64,
    pub frame_span: Option<f64>,
}

#[derive(Clone, Copy, Debug)]
pub struct Context<'a> {
    modules: &'a [ModuleInfo],
    values: Option<ModuleValues>,
}

impl<'a> Context<'a> {
    pub fn new(modules: &'a [ModuleInfo], values: Option<ModuleValues>) -> Self {
        Self { modules, values }
    }
    pub fn module(&self, id: u8) -> &ModuleInfo {
        &self.modules[id as usize]
    }
    pub fn opt_fit(&self, id: u8, opt: [u8; OPTS]) -> [usize; OPTS] {
        let o = self.module(id).options;
        std::array::from_fn(|k| o.get(k).map_or(0, |x| (opt[k] as usize).min(x.choices.len().saturating_sub(1))))
    }
    pub fn bit_time(&self, id: u8) -> f64 {
        self.values
            .filter(|v| v.id == id)
            .expect("the adapter supplies evaluated values for the selected module")
            .bit_time
    }
    pub fn frame_span(&self, id: u8) -> Option<f64> {
        self.values
            .filter(|v| v.id == id)
            .expect("the adapter supplies evaluated values for the selected module")
            .frame_span
    }
    pub fn protocols(&self) -> impl Iterator<Item = Proto> + '_ {
        BUILTIN_PROTOS.iter().copied().chain((0..self.modules.len()).map(|i| Proto::Module(i as u8)))
    }
}

/// Serial protocol of the decoder.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Proto {
    Uart,
    I2c,
    Spi,
    Can,
    Lin,
    I2s,
    Mil1553,
    Parallel,
    CanFd,
    FlexRay,
    ModbusRtu,
    ModbusAscii,
    OneWire,
    Ds2482,
    Uart9,
    Swd,
    Jtag,
    Qspi,
    I3c,
    Arinc429,
    Sent,
    /// USB 1.x / 2.0 low / full speed
    Usb,
    Manchester,
    Mlt3,
    /// MIPI C-PHY: A/B/C electrical states and synchronized 16-bit words.
    Cphy,
    /// a protocol module (`protocols::MODULES[i]`)
    Module(u8),
}

/// The protocols wired by hand; the modules come after them in [`Context::protocols`].
pub const BUILTIN_PROTOS: [Proto; 25] = [
    Proto::Uart,
    Proto::I2c,
    Proto::Spi,
    Proto::Can,
    Proto::Lin,
    Proto::I2s,
    Proto::Mil1553,
    Proto::Parallel,
    Proto::CanFd,
    Proto::FlexRay,
    Proto::ModbusRtu,
    Proto::ModbusAscii,
    Proto::OneWire,
    Proto::Ds2482,
    Proto::Uart9,
    Proto::Swd,
    Proto::Jtag,
    Proto::Qspi,
    Proto::I3c,
    Proto::Arinc429,
    Proto::Sent,
    Proto::Usb,
    Proto::Manchester,
    Proto::Mlt3,
    Proto::Cphy,
];

impl Proto {
    pub fn key_in(self, ctx: &Context<'_>) -> String {
        match self {
            Proto::Module(i) => ctx.module(i).key.to_string(),
            _ => format!("{:?}", self),
        }
    }
    pub fn from_key_in(key: &str, ctx: &Context<'_>) -> Option<Proto> {
        ctx.protocols().find(|p| p.key_in(ctx) == key)
    }
    pub fn name_in(self, ctx: &Context<'_>) -> &'static str {
        match self {
            Proto::Module(i) => ctx.module(i).name,
            Proto::Uart => "UART",
            Proto::I2c => "I2C",
            Proto::Spi => "SPI",
            Proto::Can => "CAN",
            Proto::Lin => "LIN",
            Proto::I2s => "I2S",
            Proto::Mil1553 => "1553",
            Proto::Parallel => "Parallel",
            Proto::CanFd => "CAN-FD",
            Proto::FlexRay => "FlexRay",
            Proto::ModbusRtu => "Modbus RTU",
            Proto::ModbusAscii => "Modbus ASCII",
            Proto::OneWire => "1-Wire",
            Proto::Ds2482 => "DS2482",
            Proto::Uart9 => "UART 9-bit",
            Proto::Swd => "SWD",
            Proto::Jtag => "JTAG",
            Proto::Qspi => "Quad-SPI",
            Proto::I3c => "I3C",
            Proto::Arinc429 => "ARINC 429",
            Proto::Sent => "SENT",
            Proto::Usb => "USB",
            Proto::Manchester => "Manchester",
            Proto::Mlt3 => "MLT-3",
            Proto::Cphy => "C-PHY",
        }
    }
    /// Role names of the sources (main, second, third optional, fourth optional); None = not used.
    pub fn roles_in(self, ctx: &Context<'_>) -> [Option<&'static str>; 4] {
        match self {
            Proto::Module(i) => ctx.module(i).roles,
            Proto::Cphy => [Some("A"), Some("B"), Some("C"), None],
            Proto::I2c | Proto::I3c | Proto::Ds2482 => [Some("SDA"), Some("SCL"), None, None],
            Proto::Spi => [Some("MOSI"), Some("Clock"), Some("CS"), None],
            Proto::I2s => [Some("SD"), Some("SCK"), Some("WS"), None],
            Proto::Parallel => [Some("D0"), None, Some("Clock"), None],
            Proto::Swd => [Some("SWDIO"), Some("SWCLK"), None, None],
            Proto::Jtag => [Some("TMS"), Some("TCK"), Some("TDI"), Some("TDO")],
            Proto::Qspi => [Some("IO0"), Some("SCK"), Some("CS#"), Some("IO1")],
            // D- optional: D+ alone decodes without SE0
            Proto::Usb => [Some("D+"), None, Some("D-"), None],
            _ => [Some("Source"), None, None, None],
        }
    }
    /// Serial protocols on one line that can also be decoded from a differential pair (RS-485: A - B).
    pub fn diff_ok_in(self, ctx: &Context<'_>) -> bool {
        if let Proto::Module(i) = self {
            return ctx.module(i).diff_ok;
        }
        matches!(
            self,
            Proto::Uart
                | Proto::Uart9
                | Proto::ModbusRtu
                | Proto::ModbusAscii
                | Proto::Can
                | Proto::CanFd
                | Proto::FlexRay
                | Proto::Lin
                | Proto::Arinc429
        )
    }
}
