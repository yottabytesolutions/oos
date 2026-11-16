// SPDX-License-Identifier: GPL-2.0-or-later

//! Live normalization bridge plus observe-only classification, diagnostics and requests.

pub(crate) mod acquisition;
pub(super) mod census;
mod diagnostics;
mod intake;
pub(crate) use intake::intake;
pub(super) use intake::poll;
mod legacy;
mod legacy_keys;
pub(super) mod normalize;
pub(super) mod observations;
pub(super) mod oracle;
mod picture;
pub(super) mod proposals;
mod recovery;
pub(super) mod transitions;

use super::*;
use diagnostics::Diagnostics;
use oos_core::change::{ChangeSet, Classes as C, Command, Layout, Outbox, Request, Shadow, UiAcq};
use oracle::Row;
use std::time::Instant;

#[derive(Clone, Copy)]
struct Signals {
    effects: C,
    row: Row,
    rows: u32,
    effects_by_row: [C; oracle::ROW_COUNT],
    marked_in_stop: bool,
    normalized: C,
    required: C,
}
impl Default for Signals {
    fn default() -> Self {
        Self {
            effects: C::NONE,
            row: Row::Drawing,
            rows: 0,
            effects_by_row: [C::NONE; oracle::ROW_COUNT],
            marked_in_stop: false,
            normalized: C::NONE,
            required: C::NONE,
        }
    }
}
struct Prepared {
    changes: ChangeSet,
    fields: [Option<oos_core::change::FieldDiff>; oos_core::change::FIELD_COUNT],
    signals: Signals,
    stopped: bool,
    run_edge: bool,
    single: bool,
    workers: bool,
}

const _: () = assert!(std::mem::size_of::<Prepared>() < 16 * 1024);

#[derive(Default)]
pub(super) struct Observer {
    shadow: Option<Shadow>,
    executing: bool,
    pending: Outbox,
    proposals: Outbox,
    signals: Signals,
    prepared: Option<Prepared>,
    pub(super) diagnostics: Diagnostics,
    pub(super) last: Option<ChangeSet>,
    stages: proposals::Stages,
    legacy: Option<legacy_keys::Keys>,
    timing: crate::perfguard::StageTiming,
    census: census::Census,
}

fn needs(ui: &Ui, keys: &legacy_keys::Keys) -> UiAcq {
    let size = |p: &Phosphor| (p.w, p.h);
    let layout = Layout::new(
        size(&ui.ph),
        size(&ui.phz),
        (ui.scr.w as usize, ui.scr.h as usize),
        ui.hdmi_out.map(|(w, h)| (w as usize, h as usize)),
        ui.pane_h() as usize,
        0,
    )
    .with_vertical_zoom(ui.zoom_vfactor, ui.zoom_vcenter);
    let format = if ui.src.is_hw() && !ui.needs_raw() { keys.compress() } else { 1 };
    UiAcq::of(ui.needs_raw(), ui.fast_ok(), ui.auto.is_some(), ui.bode.running, layout)
        .with_read_format(format)
        .with_mixed_bus(ui.mixed_bus_mode())
}

pub(super) fn initialize(ui: &mut Ui) {
    ui.relayout();
    let config = crate::model::config();
    let keys = legacy_keys::Keys::of(ui);
    let shadow = Shadow::new(&ui.m, needs(ui, &keys), &config);
    let observer = &mut ui.host.runtime_mut().change;
    observer.legacy = Some(keys);
    observer.signals = Signals::default();
    match shadow {
        Ok(shadow) => observer.shadow = Some(shadow),
        Err(error) => observer.diagnostics.error(error),
    }
}

pub(super) fn event(ui: &mut Ui, row: Row, effects: C) {
    let marked_in_stop = row == Row::Mark && ui.m.run == RunState::Stop;
    let observer = &mut ui.host.runtime_mut().change;
    observer.signals.marked_in_stop |= marked_in_stop;
    observer.signals.effects |= effects;
    observer.signals.effects_by_row[row as usize] |= effects;
    observer.signals.rows |= 1 << row as usize;
    // Keep the most specific observation over generic marks/resets.
    if matches!(observer.signals.row, Row::Drawing | Row::Mark | Row::Reset) || !matches!(row, Row::Mark | Row::Reset) {
        observer.signals.row = row;
    }
}

pub(super) fn request(ui: &mut Ui, request: Request, row: Row) {
    event(ui, row, C::NONE);
    let observer = &mut ui.host.runtime_mut().change;
    if let Err(error) = observer.pending.push(Command::Request(request)) {
        observer.diagnostics.error(error);
    }
}

pub(super) fn begin_step(ui: &mut Ui, dt: f64) -> Option<(bool, bool)> {
    ui.host.runtime_mut().change.executing = true;
    let t = Instant::now();
    if !lifecycle::begin_step(ui, dt) {
        ui.host.runtime_mut().change.executing = false;
        return None;
    }
    let intake = t.elapsed();
    ui.host.runtime_mut().change.timing.begin_change();
    let t = Instant::now();
    legacy::settings(ui);
    let final_done = transitions::apply(ui);
    acquisition::publish(ui);
    let timing = &mut ui.host.runtime_mut().change.timing;
    let shadow = timing.end_change();
    timing.add(intake, t.elapsed().saturating_sub(shadow));
    timing.shadow(shadow);
    let t = Instant::now();
    let cleared = picture::apply(ui);
    if cleared {
        ui.host.runtime_mut().change.timing.picture(t.elapsed());
    }
    observe(ui);
    Some((final_done, cleared))
}

pub(super) fn prepare(ui: &mut Ui) {
    let sample = ui.host.runtime().change.census.begin();
    let started = Instant::now();
    prepare_inner(ui);
    proposals::prepared(ui);
    proposals::resolve(ui);
    let observer = &mut ui.host.runtime_mut().change;
    observer.timing.record_shadow(started.elapsed());
    sample.finish(&mut observer.census, 0);
}

fn prepare_inner(ui: &mut Ui) {
    let run_edge = transitions::prepare(ui);
    let config = crate::model::config();
    let keys = legacy_keys::Keys::of(ui);
    let needs = needs(ui, &keys);
    let stopped = ui.m.run == RunState::Stop;
    let observer = &mut ui.host.runtime_mut().change;
    let transition =
        observer.pending.entries().iter().any(|c| matches!(c, Command::Request(Request::Run | Request::Single)));
    if observer.legacy.as_ref().is_some_and(|old| old.changed(&keys, transition)) {
        observer.signals.effects |= C::ACQ;
        observer.signals.effects_by_row[Row::Publication as usize] |= C::ACQ;
        observer.signals.rows |= 1 << Row::Publication as usize;
    }
    if let Some(old) = &observer.legacy {
        keys.observe(old, &mut observer.signals);
    }
    observer.legacy = Some(keys);
    if !recovery::initialize(observer, &ui.m, needs, &config) {
        return;
    }
    let Some(shadow) = &mut observer.shadow else { return };
    if shadow.needs().layout() != needs.layout() {
        observer.signals.required |= C::MAPPING;
        observer.signals.effects |= C::MAPPING;
        observer.signals.effects_by_row[Row::Layout as usize] |= C::MAPPING;
        observer.signals.rows |= 1 << Row::Layout as usize;
    }
    let mut changed = [None; oos_core::change::FIELD_COUNT];
    let mut requests = [Request::Clear; 64];
    let n = observer.pending.requests(&mut requests);
    let result = shadow.observe_with(&ui.m, needs, &requests[..n], &config, |f| {
        if f.changed() {
            changed[f.index()] = Some(f);
        }
    });
    compare_normalized(observer, &ui.m);
    match result {
        Ok(c) => {
            observer.pending.clear();
            observer.prepared = Some(Prepared {
                changes: c,
                fields: changed,
                signals: std::mem::take(&mut observer.signals),
                stopped,
                run_edge,
                workers: true,
                single: requests[..n].contains(&Request::Single),
            })
        }
        Err(error) => recovery::reject(observer, error),
    }
    // New legacy internal writers run after this point. Their tokens and observations stay queued until next step.
}

fn observe(ui: &mut Ui) {
    let sample = ui.host.runtime().change.census.begin();
    let started = Instant::now();
    let observer = &mut ui.host.runtime_mut().change;
    if let Some(prepared) = observer.prepared.take() {
        compare(observer, prepared);
    }
    observer.timing.shadow(started.elapsed());
    sample.finish(&mut observer.census, 1);
}

pub(super) fn finish_step(ui: &mut Ui) {
    ui.host.runtime_mut().change.executing = false;
    let sample = ui.host.runtime().change.census.begin();
    let started = Instant::now();
    let observer = &mut ui.host.runtime_mut().change;
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
    observer.timing.shadow(started.elapsed());
    sample.finish(&mut observer.census, 2);
}

pub(crate) fn text(ui: &Ui) -> String {
    let observer = &ui.host.runtime().change;
    use std::fmt::Write;
    let mut text = observer.diagnostics.text();
    if let Some(c) = &observer.last {
        let id = c.identity();
        let _ = write!(text, " gen={} epoch={} stats_gen={}", id.gen(), id.epoch(), id.stats_gen());
        for (token, epoch) in c.resolutions() {
            let _ = write!(text, " resolved={:?}/{}/{}:{}", token.requester(), token.run(), token.stage(), epoch);
        }
    }
    text.push_str(&observer.timing.stage_text());
    text.push_str(&crate::drawpipe::counters().losses.text());
    text.push_str(&crate::drawpipe::counters().acquisition.text());
    let [autoset, bode] = proposals::restart_counts(ui);
    let _ = write!(text, " stage_restart_autoset={autoset} stage_restart_bode={bode}");
    text.push_str(&observer.timing.diagnostics_text());
    text
}

pub(super) fn stage(ui: &mut Ui, requester: oos_core::change::Requester) {
    proposals::stage(ui, requester);
}

pub(crate) fn single(ui: &mut Ui) {
    request(ui, Request::Single, Row::Single);
    event(ui, Row::Single, C::PHOSPHOR);
}

pub(super) fn single_key(ui: &mut Ui) {
    single(ui);
    ui.m.trig_mode = TrigMode::Single;
    ui.m.run = RunState::Run;
}
pub(super) fn autoset_mark(ui: &mut Ui) {
    ui.mark_changed();
}
pub(super) fn autoset_stage(ui: &mut Ui) {
    stage(ui, oos_core::change::Requester::Autoset);
    autoset_mark(ui);
}
pub(super) fn bode_epoch(ui: &mut Ui) {
    event(ui, Row::Stage, C::VIEW);
    stage(ui, oos_core::change::Requester::Bode);
}

#[cfg(test)]
pub(super) fn counters(ui: &Ui) -> ([u64; 10], u64, u64, u64, u64) {
    let d = &ui.host.runtime().change.diagnostics;
    (d.raw, d.normalized, d.outbox, d.errors, d.unreviewed)
}
#[cfg(test)]
pub(super) fn reset(ui: &mut Ui) {
    ui.host.runtime_mut().change = Observer::default();
    initialize(ui);
}
#[cfg(test)]
pub(super) fn inject(ui: &mut Ui) {
    let mut different = ui.m.clone();
    different.ch[0].offset = 1e6;
    let config = crate::model::config();
    let observer = &mut ui.host.runtime_mut().change;
    observer.shadow.as_mut().unwrap().observe(&different, UiAcq::default(), &[Request::Clear], &config).unwrap();
    observer.pending.push(Command::Run(RunState::Stop)).unwrap();
}

#[cfg(test)]
pub(super) fn timing(ui: &Ui) -> crate::perfguard::StageTiming {
    ui.host.runtime().change.timing.clone()
}

pub(super) fn autoset_begin(ui: &mut Ui) {
    proposals::begin(ui, oos_core::change::Requester::Autoset);
    autoset_stage(ui);
}
pub(super) fn bode_begin(ui: &mut Ui) {
    ui.bode.no_phase = None;
    proposals::begin(ui, oos_core::change::Requester::Bode);
}

pub(super) fn mask_reset(ui: &mut Ui) {
    request(ui, Request::MaskReset, Row::Mask);
    ui.mask_counts_reset();
}

fn compare(observer: &mut Observer, prepared: Prepared) {
    let Prepared { changes, fields, signals, stopped, single, .. } = prepared;
    let geometry = fields.iter().flatten().any(|f| oracle::geometry_field(f.name()))
        || signals.rows & (1 << Row::Layout as usize) != 0;
    let persistence = fields.iter().flatten().any(|f| oracle::persistence_field(f.name())) || single;
    let old = oracle::dirty_contracts(signals.effects, geometry, persistence);
    let by_row = signals.effects_by_row.map(|old| oracle::dirty_contracts(old, geometry, persistence));
    let difference = observer.diagnostics.compare(
        changes.classes(),
        old,
        signals.rows,
        &by_row,
        oracle::OracleContext::new(
            stopped || signals.marked_in_stop,
            signals.normalized,
            signals.required
                | if fields.iter().flatten().any(|f| oracle::mapping_field(f.name())) { C::MAPPING } else { C::NONE }
                | if persistence { C::PHOSPHOR } else { C::NONE },
        ),
    );
    if difference != 0 {
        for field in fields.into_iter().flatten() {
            observer.diagnostics.field(field, signals.row, difference);
        }
    }
    observer.last = Some(changes);
}

pub(super) fn recall(ui: &mut Ui) {
    ui.mstat.clear();
    event(ui, Row::Recall, C::MAPPING | C::STATS);
}

fn compare_normalized(observer: &mut Observer, live: &Model) {
    let Some(shadow) = &observer.shadow else { return };
    let mut unequal = false;
    oos_core::change::fields(shadow.normalized(), live, |f| {
        if f.changed() && !matches!(f.name(), "status" | "hw_stamp" | "hw_avg_clear") {
            unequal = true;
            observer.diagnostics.field(f, observer.signals.row, oracle::Difference::D3.mask());
        }
    });
    observer.diagnostics.normalized += u64::from(unequal);
}

pub(crate) fn run_request(ui: &mut Ui) {
    request(ui, Request::Run, Row::Run);
}

#[cfg(test)]
pub(super) fn late_stage(ui: &mut Ui) -> oos_core::change::Token {
    prepare(ui);
    ui.m.tscale += 1;
    stage(ui, oos_core::change::Requester::Autoset);
    let token = ui
        .host
        .runtime()
        .change
        .pending
        .entries()
        .iter()
        .find_map(|c| if let Command::Request(Request::Epoch(t)) = c { Some(*t) } else { None })
        .unwrap();
    observe(ui);
    token
}

#[cfg(test)]
pub(super) fn duplicate_requests(ui: &mut Ui) -> usize {
    let token = oos_core::change::Token::new(oos_core::change::Requester::Autoset, 99, 1);
    let observer = &mut ui.host.runtime_mut().change;
    for _ in 0..2 {
        observer.pending.push(Command::Request(Request::Epoch(token))).unwrap();
    }
    observer.pending.entries().len()
}
#[cfg(test)]
pub(super) fn pending_count(ui: &Ui) -> usize {
    ui.host.runtime().change.pending.entries().len()
}

pub(super) fn stats_clear(ui: &mut Ui) {
    ui.clear_measurement_statistics();
    event(ui, Row::Measurement, C::STATS);
}
pub(super) fn stats_reset(ui: &mut Ui) {
    request(ui, Request::StatsReset, Row::Measurement);
    stats_clear(ui);
}

pub(super) fn replay(ui: &mut Ui) {
    request(ui, Request::Replay, Row::Layout);
    event(ui, Row::Layout, C::MAPPING);
    ui.host.runtime_mut().change.signals.required |= C::MAPPING;
}

pub(super) fn mapping_observed(ui: &mut Ui, row: Row) {
    event(ui, row, C::MAPPING);
}

pub(super) fn is_prepared(ui: &Ui) -> bool {
    ui.host.runtime().change.prepared.is_some()
}
