// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_core::change::{Requester, Token};

#[derive(Default)]
pub(super) struct Stages {
    runs: [u64; 2],
    stages: [u64; 2],
    stamps: [Option<u64>; 2],
}
fn index(requester: Requester) -> usize {
    match requester {
        Requester::Autoset => 0,
        Requester::Bode => 1,
    }
}
pub(super) fn begin(ui: &mut Ui, requester: Requester) {
    let i = index(requester);
    let ids = &mut ui.host.runtime_mut().change.stages;
    ids.runs[i] += 1;
    ids.stages[i] = 0;
    ids.stamps[i] = None;
}

/// Record a legacy stage's proposed values and unique token. It remains an observation until cutover.
pub(super) fn stage(ui: &mut Ui, requester: Requester) {
    let sample = ui.host.runtime().change.census.begin();
    let started = Instant::now();
    let i = index(requester);
    let observer = &mut ui.host.runtime_mut().change;
    if observer.stages.stamps[i] == Some(ui.m.hw_stamp) {
        return;
    }
    observer.stages.stamps[i] = Some(ui.m.hw_stamp);
    observer.stages.stages[i] += 1;
    let token = Token::new(requester, observer.stages.runs[i], observer.stages.stages[i]);
    if let Some(shadow) = &mut observer.shadow {
        observer.proposals.clear();
        if let Err(error) = observer.proposals.capture(shadow.normalized(), &ui.m) {
            observer.diagnostics.error(error);
        }
        if !shadow.proposals_agree(&observer.proposals, &ui.m) {
            observer.diagnostics.outbox += 1;
        }
        observer.proposals.clear();
    }
    if let Err(error) = observer.pending.push(Command::Request(Request::Epoch(token))) {
        observer.diagnostics.error(error);
    }
    observer.signals.row = Row::Stage;
    observer.signals.rows |= 1 << Row::Stage as usize;
    observer.timing.record_shadow(started.elapsed());
    sample.finish(&mut observer.census, 3);
}
