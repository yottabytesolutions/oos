// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// Learned state for one key before a captured batch makes its first correction.
#[derive(Clone, Copy)]
pub(in crate::board::dho) struct Pattern(Learnt);

impl Pattern {
    pub(in crate::board::dho) fn parts(self) -> ([f64; P], f64, u32) {
        (self.0.l, self.0.n, self.0.misfit)
    }

    #[cfg(test)]
    pub(in crate::board::dho) fn new(l: [f64; P], n: f64, misfit: u32) -> Self {
        Self(Learnt { l, n, misfit })
    }
}

impl Cores {
    pub(in crate::board::dho) fn fixture_pattern(&self, key: Key) -> Option<Pattern> {
        self.learnt.get(&key).copied().map(Pattern)
    }

    #[cfg(test)]
    pub(in crate::board::dho) fn restore_fixture_pattern(&mut self, key: Key, pattern: Pattern) {
        self.learnt.insert(key, pattern.0);
    }

    #[cfg(test)]
    pub(in crate::board::dho) fn fixture_seed(&mut self, len: usize, sample: impl Fn(usize) -> u16) -> Option<Pattern> {
        let (offsets, samples) = estimate_at(len.min(LOOK), sample, &mut self.win)?;
        Some(Pattern::new(offsets, samples, 0))
    }

    #[cfg(test)]
    pub(in crate::board::dho) fn fixture_fit(
        &mut self,
        pattern: Pattern,
        len: usize,
        sample: impl Fn(usize) -> u16,
    ) -> Option<(usize, f64, f64)> {
        let (estimated, _) = estimate_at(len.min(LOOK), sample, &mut self.win)?;
        Some(pattern.0.fit(&estimated))
    }
}
