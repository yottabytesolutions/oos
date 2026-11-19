// SPDX-License-Identifier: GPL-2.0-or-later

/// q=24 in both formats. Reserve headroom for persistence and four-channel grading.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CountFormat {
    #[default]
    Narrow,
    Wide,
}

impl CountFormat {
    pub(crate) fn for_hits(hits: u64) -> Self {
        if hits > super::HIT_LIMIT {
            Self::Wide
        } else {
            Self::Narrow
        }
    }

    pub(crate) fn hit_limit(self) -> u64 {
        match self {
            Self::Narrow => super::HIT_LIMIT,
            Self::Wide => u32::MAX as u64 / 8 / 24,
        }
    }
}

/// Shared placement and prefix kernels, monomorphized for each storage format.
pub(crate) trait Count: Copy + Default + PartialEq {
    fn wrapping_add(self, rhs: Self) -> Self;
    fn wrapping_sub(self, rhs: Self) -> Self;
    fn saturating_add(self, rhs: Self) -> Self;
    fn weight(value: u16) -> Self;
    fn value(self) -> u32;
    fn from_value(value: u32) -> Self;
}

macro_rules! count {
    ($ty:ty) => {
        impl Count for $ty {
            #[inline(always)]
            fn wrapping_add(self, rhs: Self) -> Self {
                self.wrapping_add(rhs)
            }
            #[inline(always)]
            fn wrapping_sub(self, rhs: Self) -> Self {
                self.wrapping_sub(rhs)
            }
            #[inline(always)]
            fn saturating_add(self, rhs: Self) -> Self {
                self.saturating_add(rhs)
            }
            #[inline(always)]
            fn weight(value: u16) -> Self {
                Self::from(value)
            }
            #[inline(always)]
            fn value(self) -> u32 {
                self as u32
            }
            #[inline(always)]
            fn from_value(value: u32) -> Self {
                value as Self
            }
        }
    };
}
count!(u16);
count!(u32);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wave::Phosphor;

    #[test]
    fn wide_storage_decay_and_channel_sum_have_headroom_and_clear_restores_narrow() {
        let mut ph = Phosphor::new(2, 2);
        ph.map[0][0] = 123;
        ph.set_format(CountFormat::Wide);
        assert_eq!(ph.count(0, 0), 123, "upgrading persistence keeps its counts");
        assert!(ph.map.iter().all(Vec::is_empty));
        let maximum = (CountFormat::Wide.hit_limit() * 24) as u32;
        for c in 0..4 {
            ph.wide_map_mut(c)[0] = maximum;
        }
        assert_eq!(ph.merged_max([true; 4]), maximum * 4);
        ph.decay(0.5);
        assert_eq!(ph.count(0, 0), maximum / 2);
        ph.halve();
        assert_eq!(ph.count(0, 0), maximum / 4);
        ph.clear();
        assert!(!ph.is_wide());
        assert!(ph.map.iter().all(|m| m.len() == 4 && m.iter().all(|&v| v == 0)));
    }
}
