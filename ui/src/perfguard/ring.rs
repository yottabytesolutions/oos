// SPDX-License-Identifier: GPL-2.0-or-later

/// A fixed window of the latest observations, including zeros.
#[derive(Clone, Debug)]
pub(crate) struct Ring<const N: usize = 256, T = u64> {
    values: [T; N],
    next: usize,
    len: usize,
    total: u64,
}
impl<const N: usize, T: Copy + Default> Default for Ring<N, T> {
    fn default() -> Self {
        Self { values: [T::default(); N], next: 0, len: 0, total: 0 }
    }
}
impl<const N: usize, T> Ring<N, T> {
    pub(crate) fn add(&mut self, value: T) {
        self.values[self.next] = value;
        self.next = (self.next + 1) % N;
        self.len = (self.len + 1).min(N);
        self.total = self.total.saturating_add(1);
    }

    pub(crate) fn samples(&self) -> &[T] {
        &self.values[..self.len]
    }
}

impl<const N: usize, const C: usize> Ring<N, [Option<u64>; C]> {
    pub(crate) fn quantiles(&self, column: usize) -> [u64; 2] {
        let mut values = [0; N];
        let mut len = 0;
        for value in self.samples().iter().filter_map(|row| row[column]) {
            values[len] = value;
            len += 1;
        }
        if len == 0 {
            return [0; 2];
        }
        values[..len].sort_unstable();
        [50, 90].map(|p| values[rank(len, p)])
    }
}

impl Ring {
    /// Nearest-rank quantiles; a tail is pending until 50 observations exist.
    pub(crate) fn format(mut self, label: &str, unit: &str) -> String {
        self.values[..self.len].sort_unstable();
        let quantile = |percent: usize| {
            if self.len == 0 || (percent == 99 && self.len < 50) {
                "pending".into()
            } else {
                self.values[rank(self.len, percent)].to_string()
            }
        };
        format!(
            " diag {label} n={} total={} p50_{unit}={} p90_{unit}={} p99_{unit}={} max_{unit}={}",
            self.len,
            self.total,
            quantile(50),
            quantile(90),
            quantile(99),
            self.values[..self.len].last().unwrap_or(&0)
        )
    }
}

fn rank(len: usize, percent: usize) -> usize {
    (len * percent).div_ceil(100) - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_partial_and_ready_use_nearest_rank_including_zero() {
        let mut ring = Ring::default();
        assert_eq!(
            ring.clone().format("test", "ns"),
            " diag test n=0 total=0 p50_ns=pending p90_ns=pending p99_ns=pending max_ns=0"
        );
        for i in 0..49 {
            ring.add(i);
        }
        assert!(ring.clone().format("test", "ns").contains("p50_ns=24 p90_ns=44 p99_ns=pending max_ns=48"));
        ring.add(49);
        assert!(ring.format("test", "ns").contains("n=50 total=50 p50_ns=24 p90_ns=44 p99_ns=49 max_ns=49"));
    }

    #[test]
    fn wrap_evicts_old_outliers_and_saturation_does_not_freeze_cursor() {
        let mut ring = Ring::default();
        ring.add(u64::MAX);
        for i in 1..=400 {
            ring.add(i);
        }
        assert!(ring
            .clone()
            .format("test", "bytes")
            .contains("n=256 total=401 p50_bytes=272 p90_bytes=375 p99_bytes=398 max_bytes=400"));
        ring.total = u64::MAX;
        for _ in 0..256 {
            ring.add(0);
        }
        assert_eq!(ring.total, u64::MAX);
        assert!(ring.format("test", "ns").contains("p50_ns=0 p90_ns=0 p99_ns=0 max_ns=0"));
    }
}
