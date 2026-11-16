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
    /// Every protocol of the Decode menu, in the order of the registry.
    pub fn protocols(&self) -> impl Iterator<Item = Proto> + '_ {
        (0..self.modules.len()).map(|i| Proto::Module(i as u8))
    }
    /// The default option settings of module `id`.
    pub fn opt_default(&self, id: u8) -> [u8; OPTS] {
        let o = self.module(id).options;
        std::array::from_fn(|k| o.get(k).map_or(0, |x| x.default as u8))
    }
}

/// Protocol of the decoder: a protocol module of the registry (`Context`). The core decoders stand first at fixed
/// places (`Proto::UART` ...); the code that knows them by name uses those.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Proto {
    Module(u8),
}

impl Proto {
    pub const UART: Proto = Proto::Module(0);
    pub const I2C: Proto = Proto::Module(1);
    pub const SPI: Proto = Proto::Module(2);
    pub const CAN: Proto = Proto::Module(3);
    pub const LIN: Proto = Proto::Module(4);
    pub const PARALLEL: Proto = Proto::Module(5);
    pub const CPHY: Proto = Proto::Module(6);

    /// The module index in the registry.
    pub fn id(self) -> u8 {
        let Proto::Module(i) = self;
        i
    }
    /// Name in saved setups: the module's key.
    pub fn key_in(self, ctx: &Context<'_>) -> String {
        ctx.module(self.id()).key.to_string()
    }
    pub fn from_key_in(key: &str, ctx: &Context<'_>) -> Option<Proto> {
        ctx.protocols().find(|p| p.key_in(ctx) == key)
    }
    pub fn name_in(self, ctx: &Context<'_>) -> &'static str {
        ctx.module(self.id()).name
    }
    /// Role names of the sources (main, second, third optional, fourth optional); None = not used.
    pub fn roles_in(self, ctx: &Context<'_>) -> [Option<&'static str>; 4] {
        ctx.module(self.id()).roles
    }
    /// Serial protocols on one line that can also be decoded from a differential pair (RS-485: A - B).
    pub fn diff_ok_in(self, ctx: &Context<'_>) -> bool {
        ctx.module(self.id()).diff_ok
    }
}
