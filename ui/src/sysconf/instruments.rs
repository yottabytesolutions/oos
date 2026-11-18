// SPDX-License-Identifier: GPL-2.0-or-later
//! Scope instrument transports with independent enablement.
#[derive(Clone, Debug, PartialEq)]
pub struct InstrumentServices {
    pub(crate) vxi11: bool,
    pub(crate) hislip: bool,
}
impl Default for InstrumentServices {
    fn default() -> Self {
        Self { vxi11: true, hislip: false }
    }
}
impl InstrumentServices {
    pub(super) fn parse(&mut self, key: &str, value: &str) -> bool {
        let Some(on) = super::on_off(value) else { return false };
        match key {
            "remote.vxi11" => self.vxi11 = on,
            "remote.hislip" => self.hislip = on,
            _ => return false,
        }
        true
    }
}
