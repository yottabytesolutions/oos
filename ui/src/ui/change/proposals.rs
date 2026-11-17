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
    stop_pending: bool,
    accepted: bool,
}
impl Stages {
    pub(super) fn quarantine(&mut self) {
        self.commands.clear();
        self.accepted = false;
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
    let started = now(ui);
    let i = index(requester);
    let observer = &mut ui.host.runtime_mut().change;
    observer.stages.stages[i] += 1;
    let token = Token::new(requester, observer.stages.runs[i], observer.stages.stages[i]);
    observer.stages.tokens[i] = Some(token);
    observer.stages.resolved[i] = None;
    if observer.executing && !capture(observer, &mut ui.m) {
        return;
    }
    if let Err(error) = observer.pending.push(Command::Request(Request::Epoch(token))) {
        observer.diagnostics.error(error);
    }
    observer.signals.row = Row::Stage;
    observer.signals.rows |= 1 << Row::Stage as usize;
    observer.timing.record_shadow(observer.timing.since(started));
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
    let emitted = !observer.stages.commands.entries().is_empty() || observer.stages.stop_pending;
    observer.stages.commands.apply(&mut ui.m);
    observer.stages.commands.clear();
    // A completion remains authoritative even if a later rejected proposal quarantined its slot.
    if std::mem::take(&mut observer.stages.stop_pending) {
        Command::Run(RunState::Stop).apply(&mut ui.m);
    }
    if emitted {
        event(ui, Row::Stage, C::NONE);
    }
}

pub(super) fn restart_counts(ui: &Ui) -> [u64; 2] {
    ui.host.runtime().change.stages.restarts
}

pub(in crate::ui) fn take_autoset(ui: &mut Ui) -> Option<super::super::autoset::AutoRun> {
    resolved(ui, Requester::Autoset)?;
    ui.auto.take()
}

pub(in crate::ui) fn autoset(ui: &mut Ui) {
    if ui.auto.is_none() {
        return;
    }
    defer(ui, Ui::auto_step);
}

pub(in crate::ui) fn bode(ui: &mut Ui) {
    if !ui.bode.running {
        return;
    }
    defer(ui, Ui::bode_update);
}

/// Complete legacy internal writers against the accepted snapshot, then return only commands.
/// The two existing detector snapshots and bounded outbox are reused; outputs stay live.
pub(in crate::ui) fn defer(ui: &mut Ui, writer: impl FnOnce(&mut Ui)) {
    if !ui.host.runtime().change.stages.accepted {
        return;
    }
    debug_assert!(
        {
            let observer = &ui.host.runtime().change;
            let mut agrees = true;
            if let Some(shadow) = &observer.shadow {
                oos_core::change::fields(shadow.accepted(), &ui.m, |f| {
                    // Status and hardware counters are outputs, not proposed settings.
                    agrees &= !f.changed() || matches!(f.name(), "status" | "hw_stamp" | "hw_avg_clear");
                });
            }
            agrees
        },
        "internal writer must enter with accepted settings"
    );
    writer(ui);
    let observer = &mut ui.host.runtime_mut().change;
    capture(observer, &mut ui.m);
}

fn capture(observer: &mut Observer, model: &mut Model) -> bool {
    let Some(shadow) = &mut observer.shadow else { return false };
    let result = observer.stages.commands.capture(shadow.accepted(), model);
    if let Err(error) = result {
        // Reusing a rejected writer's oversized capacity would prevent all following preparations.
        let outputs = (model.status, model.hw_avg_clear);
        *model = shadow.accepted().clone();
        (model.status, model.hw_avg_clear) = outputs;
        super::recovery::reject(observer, error);
        return false;
    }
    observer.proposals.clear();
    if let Err(error) = observer.proposals.capture(shadow.normalized(), model) {
        observer.diagnostics.error(error);
    }
    if !shadow.proposals_agree(&observer.proposals, model) {
        observer.diagnostics.outbox += 1;
    }
    observer.proposals.clear();
    let outputs = (model.status, model.hw_avg_clear);
    oos_core::change::refresh(model, shadow.accepted());
    (model.status, model.hw_avg_clear) = outputs;
    true
}

/// Keep completion local until the next intake applies STOP before external inputs.
pub(in crate::ui) fn stop(ui: &mut Ui) {
    let observer = &mut ui.host.runtime_mut().change;
    if observer.stages.stop_pending {
        return;
    }
    observer.stages.stop_pending = true;
    if let Err(error) = observer.stages.commands.push(Command::Run(RunState::Stop)) {
        observer.diagnostics.error(error);
    }
}

pub(in crate::ui) fn stopping(ui: &Ui) -> bool {
    ui.host.runtime().change.stages.stop_pending
}

pub(in crate::ui) fn acquiring(ui: &Ui) -> bool {
    !stopping(ui) && (ui.m.run == RunState::Run || ui.stop_final.is_some() || (ui.last[0].is_none() && !ui.cleared))
}

/// Free Bode consumes the completed SINGLE record after its deferred STOP has been accepted.
pub(in crate::ui) fn stopped(ui: &mut Ui) {
    if ui.m.run == RunState::Stop && ui.bode.running && ui.bode.free {
        bode(ui);
    }
}

pub(super) fn prepared(ui: &mut Ui) {
    let observer = &mut ui.host.runtime_mut().change;
    observer.stages.accepted = observer.prepared.is_some();
}
