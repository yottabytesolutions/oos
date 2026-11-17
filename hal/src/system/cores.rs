// SPDX-License-Identifier: GPL-2.0-or-later
//! Core capacity discovery for big.LITTLE machines.

use std::path::Path;

/// (cpu, capacity) of every CPU under `root` (`/sys/devices/system/cpu`) that has a `cpu_capacity` file.
pub fn capacities(root: &Path) -> Vec<(usize, u32)> {
    (0..64)
        .filter_map(|c| {
            std::fs::read_to_string(root.join(format!("cpu{}/cpu_capacity", c)))
                .ok()
                .and_then(|v| v.trim().parse().ok())
                .map(|v| (c, v))
        })
        .collect()
}

/// The CPUs of a big.LITTLE SoC as bit masks (bit n: CPU n), read once at start by the application: `little` the
/// cores below the largest `cpu_capacity`, `big` those at it. Both empty where the cores are all alike or the files
/// are missing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cores {
    pub big: u64,
    pub little: u64,
}

impl Cores {
    /// From (cpu, `cpu_capacity`) pairs.
    pub fn of(cap: &[(usize, u32)]) -> Cores {
        let Some(max) = cap.iter().map(|x| x.1).max() else { return Cores::default() };
        let mask = |big: bool| cap.iter().filter(|x| x.0 < 64 && (x.1 == max) == big).fold(0, |m, x| m | 1u64 << x.0);
        match mask(false) {
            0 => Cores::default(),
            little => Cores { big: mask(true), little },
        }
    }

    /// The cores of this machine (`/sys/devices/system/cpu`).
    pub fn read() -> Cores {
        Self::of(&capacities(Path::new("/sys/devices/system/cpu")))
    }

    /// The number of little cores: the deposit workers of the batch pipeline, one per little core.
    pub fn little_count(&self) -> Option<usize> {
        (self.little != 0).then(|| self.little.count_ones() as usize)
    }
}

/// The CPU numbers of a mask, ascending.
pub fn cpus(mask: u64) -> Vec<usize> {
    (0..64).filter(|c| mask >> c & 1 == 1).collect()
}
