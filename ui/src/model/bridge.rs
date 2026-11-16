// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_core::config::Config;

pub fn config() -> Config {
    let p = oos_hal::model::platform();
    Config::new(
        n_ch(),
        gen_count(),
        p.caps.la,
        std::array::from_fn(|i| board().couplings().iter().any(|c| *c as usize == i)),
        mdepths(),
        p.limits.max_srate,
        [p.limits.max_mem, p.limits.max_mem_la],
    )
    .with_catalog(crate::measures::persisted)
}

/// Whether a setup may keep catalog measurement `id`: a measurement module's own measurement.
pub fn catalog_id_valid(id: u16) -> bool {
    config().catalog_ok(id)
}

/// Transitional adapter methods. Core receives only capability and registry values.
pub trait ModelExt {
    fn n_on(&self) -> usize;
    fn max_srate(&self) -> f64;
    fn trig_coupling(&self) -> TrigCoup;
    fn la_on(&self) -> bool;
    fn trig_la_ok(&self) -> bool;
    fn pat_on_la(&self) -> bool;
    fn bus_on_la(&self) -> bool;
    fn trig_on_la(&self) -> bool;
    fn trig_line(&self) -> Option<u8>;
    fn dec_on_la(&self) -> bool;
    #[cfg(test)]
    fn max_mem(&self) -> u64;
    fn srate_depth(&self) -> (f64, u64);
    fn dec_opts(&self) -> [usize; crate::protocols::OPTS];
    fn set_dec_proto_only(&mut self, p: Proto);
    fn trig_feature(&self) -> f64;
    fn trig_span(&self) -> f64;
    fn trig_channels(&self) -> [bool; NCH];
    #[cfg(test)]
    fn depth_cap(&self) -> Option<(u64, Option<usize>)>;
    fn depth_label(&self) -> String;
    fn gen_max_hz(&self) -> f64;
    fn trig_hyst_div(&self) -> f32;
    #[cfg(test)]
    fn bus_bit(&self) -> f64;
}

impl ModelExt for Model {
    fn n_on(&self) -> usize {
        self.n_on_in(&config())
    }
    fn max_srate(&self) -> f64 {
        self.max_srate_in(&config())
    }
    fn trig_coupling(&self) -> TrigCoup {
        self.trig_coupling_in(&config())
    }
    fn la_on(&self) -> bool {
        self.la_on_in(&config())
    }
    fn trig_la_ok(&self) -> bool {
        self.trig_la_ok_in(&config())
    }
    fn pat_on_la(&self) -> bool {
        self.pat_on_la_in(&config())
    }
    fn bus_on_la(&self) -> bool {
        self.bus_on_la_in(&config())
    }
    fn trig_on_la(&self) -> bool {
        self.trig_on_la_in(&config())
    }
    fn trig_line(&self) -> Option<u8> {
        self.trig_line_in(&config())
    }
    fn dec_on_la(&self) -> bool {
        self.dec_on_la_in(&config())
    }
    #[cfg(test)]
    fn max_mem(&self) -> u64 {
        self.max_mem_in(&config())
    }
    fn srate_depth(&self) -> (f64, u64) {
        self.srate_depth_in(&config())
    }
    fn dec_opts(&self) -> [usize; crate::protocols::OPTS] {
        self.dec_opts_in(&protocols::context(self))
    }
    fn set_dec_proto_only(&mut self, p: Proto) {
        let ctx = protocols::context(self);
        self.set_dec_proto_only_in(p, &ctx)
    }
    fn trig_feature(&self) -> f64 {
        self.trig_feature_in(&protocols::context(self))
    }
    fn trig_span(&self) -> f64 {
        self.trig_span_in(&protocols::context(self))
    }
    fn trig_channels(&self) -> [bool; NCH] {
        self.trig_channels_in(&config(), &protocols::context(self))
    }
    #[cfg(test)]
    fn depth_cap(&self) -> Option<(u64, Option<usize>)> {
        self.depth_cap_in(&config(), &protocols::context(self))
    }
    fn depth_label(&self) -> String {
        self.depth_label_in(&config(), &protocols::context(self))
    }
    #[cfg(test)]
    fn bus_bit(&self) -> f64 {
        self.bus_bit_in(&protocols::context(self))
    }
    fn gen_max_hz(&self) -> f64 {
        board().gen_max_hz(&self.gen_read())
    }
    fn trig_hyst_div(&self) -> f32 {
        let c = &self.ch[self.trig_src.min(NCH - 1)];
        crate::board::board().trig_hyst_div(c.volts_per_div() as f64 / c.probe as f64, self.trig_nrej) as f32
    }
}
