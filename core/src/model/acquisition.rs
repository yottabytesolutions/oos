// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

impl Model {
    pub fn n_on_in(&self, config: &Config) -> usize {
        self.ch.iter().take(config.channels()).filter(|c| c.on).count()
    }
    /// Maximum sample rate for the number of enabled channels (1.25 GSa/s shared by the ADC cores; platform limits,
    /// provided by the application.
    pub fn max_srate_in(&self, config: &Config) -> f64 {
        let r = config.srate(self.n_on_in(config));
        // the analog rate is halved while the LA is on and a channel is shown (la.md 3.1)
        if self.la_on_in(config) && self.n_on_in(config) > 0 {
            r / 2.0
        } else {
            r
        }
    }
    /// for an LA line.
    pub fn trig_coupling_in(&self, config: &Config) -> TrigCoup {
        if self.trig_type == TrigType::Edge && self.trig_line_in(config).is_none() {
            self.trig_coup
        } else {
            TrigCoup::Dc
        }
    }

    /// A fixed memory depth the hardware cannot take for the channels acquired now: the depth applied, and the hidden
    /// trigger channel that caused it (None when the shown channels alone do). The acquisition holds the shown
    /// channels and every trigger source, shown or not (`board::dho::hw::settings`, trigger.md 2.4), and the deepest
    /// record depends on how many those are (`limits.mem`). None with Auto or when the menu's depth fits.
    pub fn depth_cap_in(&self, config: &Config, ctx: &Context<'_>) -> Option<(u64, Option<usize>)> {
        let want = config.depths().get(self.mdepth)?.1;
        let shown = (0..NCH).filter(|&c| self.ch[c].on).fold(0u32, |a, c| a | 1 << c);
        let trig = self
            .trig_channels_in(config, ctx)
            .iter()
            .enumerate()
            .filter(|(_, &u)| u)
            .fold(0u32, |a, (c, _)| a | 1 << c);
        let lim = |mask: u32| config.mem(mask.count_ones().max(1) as usize, self.la_on_in(config));
        let applied = want.min(lim(shown | trig));
        if want == 0 || applied >= want {
            return None;
        }
        let hidden = (lim(shown) >= want).then(|| (0..NCH).find(|&c| trig & !shown & 1 << c != 0)).flatten();
        Some((applied, hidden))
    }

    /// The Acquire menu's depth text: the menu's choice, and the depth applied when the channels cap it
    /// ("100M \u{2192} 62.5M (trig CH2)").
    pub fn depth_label_in(&self, config: &Config, ctx: &Context<'_>) -> String {
        let name = config.depths().get(self.mdepth).map_or("Auto", |d| d.0);
        let Some((applied, hidden)) = self.depth_cap_in(config, ctx) else { return name.into() };
        let applied =
            config.depths().iter().find(|d| d.1 == applied).map_or_else(|| applied.to_string(), |d| d.0.to_string());
        match hidden {
            Some(c) => format!("{} \u{2192} {} (trig CH{})", name, applied, c + 1),
            None => format!("{} \u{2192} {}", name, applied),
        }
    }

    pub fn la_on_in(&self, config: &Config) -> bool {
        self.la.on && config.la()
    }
    /// The trigger types that can use an LA line (the stock system also has them for D0..D15; trigger.md 5).
    pub fn trig_la_ok_in(&self, config: &Config) -> bool {
        self.la_on_in(config) && matches!(self.trig_type, TrigType::Edge | TrigType::Pulse)
    }
    /// The pattern or duration trigger looks at LA lines.
    pub fn pat_on_la_in(&self, config: &Config) -> bool {
        self.la_on_in(config)
            && matches!(self.trig_type, TrigType::Pattern | TrigType::Duration)
            && self.pat_la.iter().any(|&b| b != Bit::X)
    }
    /// The protocol trigger on LA lines (the decoder's input is the LA).
    pub fn bus_on_la_in(&self, config: &Config) -> bool {
        self.trig_type == TrigType::Bus && self.dec_on_la_in(config)
    }
    /// The hardware trigger cannot be refined on the analog samples (it is on LA lines).
    pub fn trig_on_la_in(&self, config: &Config) -> bool {
        self.trig_line_in(config).is_some() || self.pat_on_la_in(config) || self.bus_on_la_in(config)
    }
    /// The LA line the trigger is on: only while the LA is on and with a type that can use one.
    pub fn trig_line_in(&self, config: &Config) -> Option<u8> {
        self.trig_la.filter(|_| self.trig_la_ok_in(config))
    }
    /// The decoder reads logic analyser lines (the choice only counts while the LA is on).
    pub fn dec_on_la_in(&self, config: &Config) -> bool {
        self.dec_la_in && self.la_on_in(config) && self.dec_proto != Proto::CPHY
    }
    pub fn max_mem_in(&self, config: &Config) -> u64 {
        // limits 125 / 62.5 / 31.25 M (stock: 50 / 25 / 10 M); above 78.6 M only with the larger segment cap
        // (`hal::dho::hz::MEM_BURSTS`); overridable in limits.conf (`mem.1ch` ...)
        // with the LA on: 31.25 / 12.5 / 10 M (`max_mem_la`, la.md 3.2)
        config.mem(self.n_on_in(config), self.la_on_in(config))
    }
    /// (sample rate, memory depth) as the acquisition would use them.
    pub fn srate_depth_in(&self, config: &Config) -> (f64, u64) {
        let window = self.hw_time_per_div() as f64 * HDIV as f64;
        let want = config.depths()[self.mdepth].1;
        let maxsr = self.max_srate_in(config);
        if want == 0 {
            let depth_cap = 1_000_000u64.min(self.max_mem_in(config));
            let sr = maxsr.min(depth_cap as f64 / window);
            let sr = quantize_srate(sr, maxsr);
            (sr, ((sr * window).round() as u64).max(1000).min(depth_cap))
        } else {
            let depth = want.min(self.max_mem_in(config));
            let sr = quantize_srate(maxsr.min(depth as f64 / window), maxsr);
            (sr, depth)
        }
    }
}

/// Sample rates are the ADC rate divided by an integer decimation (1-2-5 steps here).
fn quantize_srate(sr: f64, max: f64) -> f64 {
    let mut best = max;
    let mut d = 1.0;
    while best > sr * 1.0001 && d < 1e12 {
        for m in [1.0, 2.0, 5.0] {
            let c = max / (d * m);
            if c <= sr * 1.0001 {
                return c;
            }
        }
        d *= 10.0;
        best = max / d;
    }
    best
}
