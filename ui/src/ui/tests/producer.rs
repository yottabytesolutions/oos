// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::*;

pub(super) trait Producer {
    fn producer_step(&mut self, dt: f64);
}

impl Producer for Ui {
    fn producer_step(&mut self, dt: f64) {
        // These legacy consumer fixtures inject freshly acquired frames, with no producer thread.
        // Dedicated epochs tests inject stale identities and call Ui::step directly.
        if self.src.is_hw() {
            let mut frames = match &self.src {
                Source::Hw { shared, .. } => std::mem::take(&mut oos_hal::sync::lock(shared).queue),
                _ => unreachable!(),
            };
            let id = change::acquisition::script_prepare(self);
            if let Source::Hw { shared, .. } = &self.src {
                let mut g = oos_hal::sync::lock(shared);
                for frame in &mut frames {
                    (frame.cfg, frame.stamp) = (id.gen(), id.epoch());
                }
                g.queue = frames;
                if let Some((_, frame)) = &mut g.seg.shown {
                    (frame.cfg, frame.stamp) = (id.gen(), id.epoch());
                }
            }
        }
        self.step(dt);
    }
}
