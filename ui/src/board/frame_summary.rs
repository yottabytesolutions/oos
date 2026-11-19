// SPDX-License-Identifier: GPL-2.0-or-later

use super::{BasicStats, NCH};
use std::sync::Arc;

/// Summaries and the backing grant of their captured waveform. Frame and result Arc
/// retention keeps this grant alive after FIFO removal and plan retirement.
#[derive(Default)]
pub struct FrameSummary {
    values: [BasicStats; NCH],
    charge: Option<Arc<crate::drawpipe::TapReservation>>,
}

impl FrameSummary {
    pub(crate) fn retain(&mut self, charge: Arc<crate::drawpipe::TapReservation>) {
        self.charge = Some(charge);
    }
}
// A Frame clone owns independently copied samples, covered by the application's copy allowance.
impl Clone for FrameSummary {
    fn clone(&self) -> Self {
        self.values.into()
    }
}
impl From<[BasicStats; NCH]> for FrameSummary {
    fn from(values: [BasicStats; NCH]) -> Self {
        Self { values, charge: None }
    }
}
impl std::ops::Deref for FrameSummary {
    type Target = [BasicStats; NCH];
    fn deref(&self) -> &Self::Target {
        &self.values
    }
}
impl std::ops::DerefMut for FrameSummary {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.values
    }
}

impl std::fmt::Debug for FrameSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrameSummary").field("values", &self.values).finish_non_exhaustive()
    }
}
