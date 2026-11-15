// SPDX-License-Identifier: GPL-2.0-or-later

//! Capability values supplied by the application. No platform detection happens here.

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    channels: usize,
    generators: usize,
    la: bool,
    couplings: [bool; 4],
    depths: &'static [(&'static str, u64)],
    rates: [f64; 3],
    memory: [[u64; 3]; 2],
}

impl Config {
    /// Slots are clamped to the domain's fixed capacity; a scope always has an analog input.
    pub fn new(
        channels: usize,
        generators: usize,
        la: bool,
        couplings: [bool; 4],
        depths: &'static [(&'static str, u64)],
        rates: [f64; 3],
        memory: [[u64; 3]; 2],
    ) -> Self {
        Self {
            channels: channels.clamp(1, oos_dsp::CHANNELS),
            generators: generators.min(1),
            la,
            couplings,
            depths,
            rates,
            memory,
        }
    }

    pub fn channels(&self) -> usize {
        self.channels
    }
    pub fn generators(&self) -> usize {
        self.generators
    }
    pub fn la(&self) -> bool {
        self.la
    }
    pub fn coupling(&self, index: usize) -> bool {
        self.couplings.get(index).copied().unwrap_or(false)
    }
    pub fn depths(&self) -> &'static [(&'static str, u64)] {
        self.depths
    }
    pub fn source_exists(&self, source: usize, slots: usize) -> bool {
        source < self.channels || source >= slots
    }
    fn index(n: usize) -> usize {
        n.saturating_sub(1).min(2)
    }
    pub fn srate(&self, n: usize) -> f64 {
        self.rates[Self::index(n)]
    }
    pub fn mem(&self, n: usize, la: bool) -> u64 {
        self.memory[usize::from(la)][Self::index(n)]
    }
}

/// Source cycling among analog and math slots, skipping absent analog inputs.
pub fn src_step_in(i: usize, n: usize, d: i32, nch: usize) -> usize {
    let v: Vec<usize> = (0..n).filter(|&k| k < nch || k >= oos_dsp::CHANNELS).collect();
    if v.is_empty() {
        return i;
    }
    let p = v.iter().position(|&k| k == i).unwrap_or(0) as i32;
    v[(p + d).rem_euclid(v.len() as i32) as usize]
}

pub fn opt_src_step_in(i: usize, d: i32, nch: usize) -> usize {
    let v: Vec<usize> = (0..=nch).collect();
    let p = v.iter().position(|&k| k == i).unwrap_or(0) as i32;
    v[(p + d).rem_euclid(v.len() as i32) as usize]
}
