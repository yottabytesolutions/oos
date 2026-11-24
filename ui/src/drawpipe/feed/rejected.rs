// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

impl Batch {
    pub(super) fn rejected_waveform(&mut self) -> Option<Arc<Frame>> {
        let fallback = self.newest.take();
        drop(self.header.order.take());
        if let Some(charge) = &self.header.tap {
            charge.fallback();
        }
        let next = self.next.get_mut();
        // The record path accounts for the returned waveform.
        *next += usize::from(fallback.is_some() && *next < self.frames);
        fallback
    }
}
