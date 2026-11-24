// SPDX-License-Identifier: GPL-2.0-or-later
use std::time::Duration;

/// One millisecond per bin, with an overflow bin at 255 ms.
pub(crate) struct Histogram([u64; 256]);

impl Default for Histogram {
    fn default() -> Self {
        Self([0; 256])
    }
}

impl Histogram {
    pub(crate) fn add(&mut self, elapsed: Duration) {
        let bin = elapsed.as_millis().min(255) as usize;
        self.0[bin] = self.0[bin].saturating_add(1);
    }

    pub(crate) fn quantiles_us(&self) -> [u64; 3] {
        let count: u128 = self.0.iter().copied().map(u128::from).sum();
        [50, 90, 99].map(|percent| {
            let rank = (count * percent).div_ceil(100);
            let mut seen = 0;
            self.0
                .iter()
                .position(|&n| {
                    seen += u128::from(n);
                    seen >= rank
                })
                .unwrap_or(255) as u64
                * 1000
        })
    }

    #[cfg(feature = "private-picture-counts")]
    pub(super) fn text(&self) -> String {
        self.0.iter().map(u64::to_string).collect::<Vec<_>>().join(",")
    }

    #[cfg(all(test, feature = "private-picture-counts"))]
    pub(super) fn bins(&self) -> &[u64; 256] {
        &self.0
    }
}
