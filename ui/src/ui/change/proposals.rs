// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use oos_core::change::{Requester, Token};

#[derive(Default)]
pub(super) struct Stages {
    runs: [u64; 2],
    stages: [u64; 2],
    tokens: [Option<Token>; 2],
    resolved: [Option<oos_core::change::Identity>; 2],
    commands: Outbox,
    restarts: [u64; 2],
}
impl Stages {
    pub(super) fn quarantine(&mut self) {
        self.commands.clear();
        self.tokens = [None; 2];
        self.resolved = [None; 2];
    }
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
    ids.tokens[i] = None;
    ids.resolved[i] = None;
}

/// Queue a unique stage token and defer late associated settings to the next change point.
pub(super) fn stage(ui: &mut Ui, requester: Requester) {
    let sample = ui.host.runtime().change.census.begin();
    let started = Instant::now();
    let i = index(requester);
    let observer = &mut ui.host.runtime_mut().change;
    observer.stages.stages[i] += 1;
    let token = Token::new(requester, observer.stages.runs[i], observer.stages.stages[i]);
    observer.stages.tokens[i] = Some(token);
    observer.stages.resolved[i] = None;
    if let Some(shadow) = &mut observer.shadow {
        if let Err(error) =
            if observer.executing { observer.stages.commands.capture(shadow.accepted(), &ui.m) } else { Ok(()) }
        {
            observer.diagnostics.error(error);
            return;
        }
        observer.proposals.clear();
        if let Err(error) = observer.proposals.capture(shadow.normalized(), &ui.m) {
            observer.diagnostics.error(error);
        }
        if !shadow.proposals_agree(&observer.proposals, &ui.m) {
            observer.diagnostics.outbox += 1;
        }
        observer.proposals.clear();
        if observer.executing {
            let outputs = (ui.m.status, ui.m.hw_avg_clear);
            oos_core::change::refresh(&mut ui.m, shadow.accepted());
            (ui.m.status, ui.m.hw_avg_clear) = outputs;
        }
    }
    if let Err(error) = observer.pending.push(Command::Request(Request::Epoch(token))) {
        observer.diagnostics.error(error);
    }
    observer.signals.row = Row::Stage;
    observer.signals.rows |= 1 << Row::Stage as usize;
    observer.timing.record_shadow(started.elapsed());
    sample.finish(&mut observer.census, 3);
}

pub(super) fn resolve(ui: &mut Ui) {
    let observer = &mut ui.host.runtime_mut().change;
    let Some(prepared) = &observer.prepared else { return };
    for i in 0..2 {
        if observer.stages.tokens[i].is_some_and(|t| prepared.changes.epoch(t).is_some()) {
            let id = prepared.changes.identity();
            observer.stages.resolved[i] = Some(id);
            if i == 0 {
                if let Some(a) = &mut ui.auto {
                    a.stamp = id.epoch();
                }
            }
        }
    }
    restart_foreign(ui);
}

fn restart_foreign(ui: &mut Ui) {
    for (i, requester) in [Requester::Autoset, Requester::Bode].into_iter().enumerate() {
        let active = if i == 0 { ui.auto.is_some() } else { ui.bode.running };
        if !active {
            continue;
        }
        let now = acquisition::identity(ui);
        let stages = &ui.host.runtime().change.stages;
        let restart = match stages.resolved[i] {
            Some(old) => (old.gen(), old.epoch()) != (now.gen(), now.epoch()),
            None => stages.tokens[i].is_none(),
        };
        if !restart {
            continue;
        }
        let count = &mut ui.host.runtime_mut().change.stages.restarts[i];
        *count += 1;
        if *count <= 4 {
            eprintln!("oos-ui stage: {requester:?} pair superseded; restarting stage");
        }
        if i == 0 {
            autoset::recovery::restart(ui);
        } else {
            ui.bode.wait = 2;
        }
        stage(ui, requester);
    }
}

pub(in crate::ui) fn resolved(ui: &Ui, requester: Requester) -> Option<oos_core::change::Identity> {
    ui.host.runtime().change.stages.resolved[index(requester)]
}

pub(in crate::ui) fn apply(ui: &mut Ui) {
    let observer = &mut ui.host.runtime_mut().change;
    observer.stages.commands.apply(&mut ui.m);
    observer.stages.commands.clear();
}

pub(super) fn restart_counts(ui: &Ui) -> [u64; 2] {
    ui.host.runtime().change.stages.restarts
}

pub(in crate::ui) fn take_autoset(ui: &mut Ui) -> Option<super::super::autoset::AutoRun> {
    resolved(ui, Requester::Autoset)?;
    ui.auto.take()
}
