// SPDX-License-Identifier: GPL-2.0-or-later

//! Isolated host census. A probe is installed only by the explicit scripted test entry.

use super::*;

#[derive(Default)]
pub(super) struct Census {
    count: Option<fn() -> usize>,
    rows: [[usize; 2]; 5],
}
pub(super) struct Sample {
    count: Option<fn() -> usize>,
    before: usize,
}
impl Census {
    pub(super) fn begin(&self) -> Sample {
        Sample { count: self.count, before: self.count.map_or(0, |count| count()) }
    }
}
impl Sample {
    pub(super) fn finish(self, census: &mut Census, row: usize) {
        if let Some(count) = self.count {
            census.rows[row][0] += 1;
            census.rows[row][1] += count() - self.before;
        }
    }
}

pub(in crate::ui) fn run(steps: usize, count: fn() -> usize) -> [[usize; 2]; 5] {
    // Fake Shared has no producer or worker threads, so a process census measures only this UI.
    let mut ui = Ui::new(Source::Hw { shared: Default::default(), seen: 0 }, host::Host::headless());
    ui.m.meas.clear();
    ui.m.meas2.clear();
    ui.m.run = RunState::Stop;
    for step in 0..100 + steps {
        if step == 100 {
            ui.host.runtime_mut().change.census.count = Some(count);
        }
        ui.m.ch[0].offset = if step % 2 == 0 { 1e6 } else { -1e6 };
        ui.step(0.04);
        // Exercise late stage commands, snapshot restoration and token capture after warming their retained slots.
        ui.host.runtime_mut().change.executing = true;
        ui.m.trig_level = if step % 2 == 0 { 0.125 } else { 0.25 };
        stage(&mut ui, oos_core::change::Requester::Autoset);
        ui.host.runtime_mut().change.executing = false;
    }
    ui.host.runtime().change.census.rows
}
