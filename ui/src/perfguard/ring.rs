// SPDX-License-Identifier: GPL-2.0-or-later

/// The latest 256 observations, including zeros. Cursor and lifetime count are independent.
#[derive(Clone, Debug)]
pub(crate) struct Ring {
    values: [u64; 256],
    next: usize,
    len: usize,
    total: u64,
}
impl Default for Ring {
    fn default() -> Self {
        Self { values: [0; 256], next: 0, len: 0, total: 0 }
    }
}
impl Ring {
    pub(crate) fn add(&mut self, value: u64) {
        self.values[self.next] = value;
        self.next = (self.next + 1) % 256;
        self.len = (self.len + 1).min(256);
        self.total = self.total.saturating_add(1);
    }

    /// Nearest-rank quantiles; a tail is pending until 50 observations exist.
    pub(crate) fn format(mut self, label: &str, unit: &str) -> String {
        self.values[..self.len].sort_unstable();
        let quantile = |percent: usize| {
            if self.len == 0 || (percent == 99 && self.len < 50) {
                "pending".into()
            } else {
                self.values[(self.len * percent).div_ceil(100) - 1].to_string()
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
