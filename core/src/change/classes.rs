// SPDX-License-Identifier: GPL-2.0-or-later

/// The ten independent reset contracts. Empty means unrelated, output, or transition.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Classes(u16);

impl Classes {
    pub const NONE: Self = Self(0);
    pub const ACQ: Self = Self(1);
    pub const MAPPING: Self = Self(2);
    pub const PHOSPHOR: Self = Self(4);
    pub const STATS: Self = Self(8);
    pub const MASK: Self = Self(16);
    pub const ZONE: Self = Self(32);
    pub const HW_AVERAGE: Self = Self(64);
    pub const HISTORY: Self = Self(128);
    pub const MATH: Self = Self(256);
    pub const VIEW: Self = Self(512);
    pub const ALL: [Self; 10] = [
        Self::ACQ,
        Self::MAPPING,
        Self::PHOSPHOR,
        Self::STATS,
        Self::MASK,
        Self::ZONE,
        Self::HW_AVERAGE,
        Self::HISTORY,
        Self::MATH,
        Self::VIEW,
    ];
    pub const NAMES: [&'static str; 10] =
        ["acq", "mapping", "phosphor", "stats", "mask", "zone", "hw_average", "history", "math", "view"];
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl std::ops::BitOr for Classes {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}
impl std::ops::BitOrAssign for Classes {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.union(rhs);
    }
}
