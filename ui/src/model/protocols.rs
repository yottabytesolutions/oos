// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_core::protocols::{Context, ModuleInfo, ModuleValues};

/// Every protocol of the Decode menu, in the order of `protocols::MODULES`.
pub static PROTOS: std::sync::LazyLock<Vec<Proto>> =
    std::sync::LazyLock::new(|| (0..crate::protocols::MODULES.len()).map(|i| Proto::Module(i as u8)).collect());

static INFO: std::sync::LazyLock<Vec<ModuleInfo>> = std::sync::LazyLock::new(|| {
    crate::protocols::MODULES
        .iter()
        .map(|p| ModuleInfo {
            key: p.key(),
            name: p.name(),
            roles: p.roles(),
            diff_ok: p.diff_ok(),
            options: p.options(),
        })
        .collect()
});

fn metadata() -> Context<'static> {
    Context::new(&INFO, None)
}

/// The registry with the bit time and frame span of the decoder's module, evaluated with its options and the shared
/// rows of the model.
pub fn context(m: &Model) -> Context<'static> {
    let id = m.dec_proto.id();
    let p = crate::protocols::get(id);
    let opt = metadata().opt_fit(id, m.dec_opt);
    let sh = crate::protocols::Shared::of(m);
    Context::new(
        &INFO,
        Some(ModuleValues { id, bit_time: p.bit_time_sh(&opt, &sh), frame_span: p.frame_span_sh(&opt, &sh) }),
    )
}

pub trait ProtoExt {
    fn module(self) -> Option<&'static dyn crate::protocols::Protocol>;
    fn key(self) -> String;
    fn from_key(k: &str) -> Option<Proto>;
    fn name(self) -> &'static str;
    fn roles(self) -> [Option<&'static str>; 4];
    fn diff_ok(self) -> bool;
}

impl ProtoExt for Proto {
    fn module(self) -> Option<&'static dyn crate::protocols::Protocol> {
        Some(crate::protocols::get(self.id()))
    }
    fn key(self) -> String {
        self.key_in(&metadata())
    }
    fn from_key(k: &str) -> Option<Proto> {
        Self::from_key_in(k, &metadata())
    }
    fn name(self) -> &'static str {
        self.name_in(&metadata())
    }
    fn roles(self) -> [Option<&'static str>; 4] {
        self.roles_in(&metadata())
    }
    fn diff_ok(self) -> bool {
        self.diff_ok_in(&metadata())
    }
}
