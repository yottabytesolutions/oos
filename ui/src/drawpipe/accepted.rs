// SPDX-License-Identifier: GPL-2.0-or-later
#[derive(Default)]
pub struct Accepted {
    pub frames: u64,
    pub triggered: u64,
    pub folds: crate::framestats::Folds,
    pub newest: Option<std::time::Instant>,
    #[cfg(feature = "private-picture-counts")]
    pub oldest: Option<std::time::Instant>,

    pub stale: u64,
    pub(crate) window_frames: Option<u64>,
}
