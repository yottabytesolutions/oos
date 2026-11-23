// SPDX-License-Identifier: GPL-2.0-or-later

use super::run_order::{edit_level, headless};
use super::*;
use crate::{
    board::PictureEligibility, drawpipe::stage::owner::Owner, sparse::Interp, ui::step_draw, wave::Phosphor, Ui,
};
use std::{collections::BTreeMap, panic::AssertUnwindSafe};

type Fields = BTreeMap<String, u64>;

fn record_fields(ui: &mut Ui) -> Fields {
    let reply = crate::scpi::Scpi::disconnected().exec_line(ui, &crate::Canvas::new(1, 1), ":SYSTem:STATe?");
    String::from_utf8(reply)
        .unwrap()
        .split_whitespace()
        .filter_map(|token| token.split_once('='))
        .filter(|(key, _)| {
            matches!(*key, "read" | "deposited" | "shown" | "records.recovered")
                || key.starts_with("lost.")
                || matches!(*key, "rejected.stale_key" | "rejected.epoch")
        })
        .filter_map(|(key, value)| value.parse().ok().map(|count| (key.to_owned(), count)))
        .collect()
}

fn assert_conservation(fields: &Fields, pictured: u64) {
    let lost: u64 = fields
        .iter()
        .filter(|(key, _)| key.starts_with("lost.") || key.starts_with("rejected."))
        .map(|(_, count)| count)
        .sum();
    assert_eq!(fields["read"], fields["deposited"] + lost, "{fields:?}");
    assert_eq!(fields["deposited"], pictured, "{fields:?}");
    let recovered = fields["records.recovered"];
    assert!(fields["read"] >= fields["deposited"] + recovered, "{fields:?}");
    assert!(recovered <= fields["shown"] && fields["shown"] <= fields["read"] + pictured, "{fields:?}");
}

#[test]
fn software_average_raw_readout_reaches_a_picture_or_reports_its_fallback() {
    for (channels, tdiv) in [(1, 1e-6), (1, 10e-6), (4, 1e-6), (4, 10e-6)] {
        assert_average_raw_readout(channels, tdiv);
    }
}

fn assert_average_raw_readout(channels: usize, tdiv: f64) {
    let scope = Scope::new();
    let intake = scope.gated_worker();
    let out = HwShared::default();
    let src = crate::wave::Source::Hw { shared: out.clone(), seen: 0 };
    let switches = Switches { compress: true, ..Default::default() };
    let mut ui = Ui::new(src, crate::ui::host::Host::headless().with_switches(switches));
    ui.m = picture::model(tdiv);
    ui.m.acq = crate::model::AcqMode::Average;
    ui.m.avg_hw = false;
    ui.m.avg_count = 16;
    for (index, ch) in ui.m.ch.iter_mut().enumerate() {
        ch.on = index < channels;
    }
    ui.step(0.0);
    let want = lock(&out).want.clone().unwrap();
    assert_eq!(want.compress, 1);
    let run = Run::Pictures(picture::StopAt::Between);
    let (mut io, _) = sim::fixture(&want, out.clone(), (run, 4, false), Some(intake));
    let (mut source, _) = source(&out, &io);
    capture(&mut source, &mut io);
    assert!(!lock(&out).record_flow.serial);
    let batch = io.clock.take_submission();
    if channels == 4 && tdiv == 1e-6 {
        assert!(batch, "the raw 4-channel sample grid fits the Average worker");
    }
    ui.step(1.0 / 30.0);
    let fields = record_fields(&mut ui);
    assert_eq!(fields["read"], 4);
    assert_eq!(fields["shown"], 4);
    assert_conservation(&fields, 4);
    let text = drawpipe::counters().pipeline_text(Duration::ZERO, 0);
    let fallback = if batch { 0 } else { 4 };
    assert!(text.contains(&format!(" average_pairs={fallback} ")), "{text}");
    #[cfg(feature = "private-picture-counts")]
    if batch {
        let pictures = drawpipe::counters().timing.picture_counts().text();
        assert!(pictures.contains(" fresh=1 "), "{pictures}");
        assert!(pictures.contains(" first_records=4 "), "{pictures}");
    }
}

fn recovered(draw: impl FnOnce() -> Ui, read: u64, deposited: u64, loss: (&str, u64)) -> Fields {
    let _scope = Scope::new();
    let mut ui = draw();
    let fields = record_fields(&mut ui);
    assert_eq!(fields["read"], read, "{fields:?}");
    assert_eq!(fields[loss.0], loss.1, "{fields:?}");
    assert_conservation(&fields, deposited);
    assert_eq!(fields["records.recovered"], 1, "{fields:?}");
    assert_eq!(fields["shown"], deposited + 1, "{fields:?}");
    ui.step(1.0 / 30.0);
    assert_eq!(record_fields(&mut ui), fields, "the same tap can only recover once");
    fields
}

#[test]
fn state_reply_conserves_records_when_ui_redraws_after_remap() {
    twice(|| {
        with_projected_batch(4, |ui, out, _, interp| {
            ui.m.dots = crate::model::DotsMode::On;
            ui.step(1.0 / 30.0);

            let latest = lock(out).frame.clone();
            let mut expected = Phosphor::new(ui.ph.w, ui.ph.h);
            drawpipe::draw_frame(&mut expected, &ui.m, &latest, interp, None);
            assert_eq!(latest.seq, 4);
            assert!(expected.maps()[0].iter().any(|&value| value != 0));
            assert!(ui.ph.maps() == expected.maps(), "the picture contains only the remapped waveform");
            let fields = record_fields(ui);
            assert_eq!((fields["read"], fields["rejected.stale_key"]), (4, 4));
            assert_conservation(&fields, 0);
            assert_eq!((fields["records.recovered"], fields["shown"]), (0, 1));
            ui.step(1.0 / 30.0);
            assert_eq!(record_fields(ui), fields, "the tap can only be counted once");
            stop(ui);
            assert_eq!(record_fields(ui), fields, "the settled STOP keeps the early draw and its separate loss");
            fields
        })
    });
}

#[test]
fn projected_draw_counts_only_as_shown_before_acceptance_or_loss() {
    for accepted in [true, false] {
        twice(|| {
            with_projected_batch(1, |ui, _, pipe, interp| {
                let changed = Phosphor::new(ui.ph.w - 1, ui.ph.h);
                let original = std::mem::replace(&mut ui.ph, changed);
                let tap = pipe.take_waveform((ui.ph.w, ui.ph.h)).unwrap();
                assert_eq!(tap.eligibility(), PictureEligibility::ShownOnly);
                step_draw::count_tests::draw_tap(ui, tap, interp);
                assert!(ui.ph.maps()[0].iter().any(|&value| value != 0));

                // Resolve the deposit independently of the early draw. Restoring its geometry lets the UI compose it.
                if accepted {
                    ui.ph = original;
                }
                let pictured = step_draw::count_tests::compose_deposit(ui, pipe);
                assert_eq!(pictured, u64::from(accepted));
                if accepted {
                    stop(ui);
                }
                let fields = record_fields(ui);
                assert_conservation(&fields, pictured);
                assert_eq!(
                    (fields["read"], fields["shown"], fields["records.recovered"]),
                    (1, 1 + u64::from(accepted), 0)
                );
                assert_eq!(fields["rejected.stale_key"], u64::from(!accepted));
                fields
            })
        });
    }
}

fn with_projected_batch(
    records: u32,
    check: impl FnOnce(&mut Ui, &HwShared, &drawpipe::Pipe, Interp) -> Fields,
) -> Fields {
    let scope = Scope::new();
    let intake = scope.gated_worker();
    let out = HwShared::default();
    let mut ui = headless(&out, false);
    let want = lock(&out).want.clone().unwrap();
    let run = Run::Pictures(picture::StopAt::Between);
    let (mut io, _) = sim::fixture(&want, out.clone(), (run, records, false), Some(intake));
    let (mut source, _) = source(&out, &io);
    let pipe = drawpipe::pipe().unwrap();
    let (w, h) = (ui.ph.w, ui.ph.h);
    let identity = lock(&out).acquisition.identity();
    pipe.publish_view(&ui.m, w, h, source.switches.interp, identity);
    capture(&mut source, &mut io);
    assert!(io.clock.take_submission());
    check(&mut ui, &out, pipe, source.switches.interp)
}

#[test]
fn state_reply_conserves_records_when_ui_draws_after_hit_budget_rejection() {
    twice(|| recovered(crate::ui::draw_routes::draw_after_hit_budget_rejection, 32, 0, ("lost.budget", 32)));
}

#[test]
fn state_reply_accounts_for_every_read_record_across_mixed_outcomes() {
    twice(mixed_outcomes);
}

fn mixed_outcomes() -> Fields {
    let scope = Scope::new();
    let intake = scope.gated_worker();
    let out = HwShared::default();
    let mut ui = headless(&out, false);
    let want = lock(&out).want.clone().unwrap();
    let run = Run::Pictures(picture::StopAt::Between);
    let (mut io, trace) = sim::fixture(&want, out.clone(), (run, 4, false), Some(intake));
    let (mut source, _) = source(&out, &io);
    source.switches.board.frame0 = crate::board::dho::switches::FrameZero::Drop;
    let (w, h) = picture::SIZE;
    let identity = lock(&out).acquisition.identity();
    drawpipe::pipe().unwrap().publish_view(&ui.m, w, h, source.switches.interp, identity);
    let request = Request::read(&lock(&out));
    let (settings, params) = source.reconfigure(&mut io, &request, &mut false).unwrap();
    short_buffer(&mut source, &mut io, (&settings, &params), &request, (4, 3));
    capture(&mut source, &mut io); // Parks until a picture releases the short batch.
    io.clock.compose();
    capture(&mut source, &mut io);
    for event in edit_level(&mut ui, &out) {
        io.clock.mark(event);
    }
    assert!(io.clock.take_submission(), "batch under the old identity");
    assert!(lock(&out).queue.is_empty(), "no record waits outside the pipeline");
    let pictured: u64 = trace
        .borrow()
        .events
        .iter()
        .filter_map(|event| if let Event::Picture(picture) = event { Some(picture.frames) } else { None })
        .sum();

    let fields = record_fields(&mut ui);
    for (name, count) in [
        ("read", 12),
        ("deposited", 3),
        ("lost.settings", 2),
        ("lost.error", 1),
        ("lost.frame0", 3),
        ("lost.frame1", 3),
    ] {
        assert_eq!(fields[name], count, "{name}: {fields:?}");
    }
    assert_conservation(&fields, pictured);
    fields
}

#[derive(Clone, Copy)]
enum ComposeLoss {
    StaleKey,
    Overflow,
    Error,
}

fn compose_loss(loss: ComposeLoss) -> Fields {
    let scope = Scope::new();
    let intake = scope.gated_worker();
    let out = HwShared::default();
    let mut ui = headless(&out, false);
    let want = lock(&out).want.clone().unwrap();
    let run = Run::Pictures(picture::StopAt::Between);
    let (mut io, _) = sim::fixture(&want, out.clone(), (run, 4, false), Some(intake));
    let (mut source, _) = source(&out, &io);
    let pipe = drawpipe::pipe().unwrap();
    let (w, h) = picture::SIZE;
    let identity = lock(&out).acquisition.identity();
    pipe.publish_view(&ui.m, w, h, source.switches.interp, identity);
    let mut owner = Owner::default();
    owner.set_wake(std::sync::Arc::new(crate::system::intake::Wake::new().unwrap()));
    let batches = if matches!(loss, ComposeLoss::Overflow) { 5 } else { 1 };
    for _ in 0..batches {
        capture(&mut source, &mut io);
        assert!(io.clock.take_submission());
        owner.intake_owned(pipe, true);
        assert!(!io.clock.take_submission(), "staging only wakes the gated worker");
    }
    let counters = drawpipe::counters();
    let (field, lost, pictured) = match loss {
        ComposeLoss::StaleKey => {
            edit_level(&mut ui, &out);
            let current = lock(&out).acquisition.identity();
            pipe.publish_view(&ui.m, w, h, source.switches.interp, current);
            ("rejected.stale_key", 4, 0)
        }
        ComposeLoss::Overflow => {
            owner.inject_excess_for_test(pipe, 2);
            drawpipe::Counters::add(&counters.acquired, 2);
            ("lost.queue_overflow", 2, 20)
        }
        ComposeLoss::Error => {
            owner.invalidate_staged_hits_for_test();
            ("lost.error", 4, 0)
        }
    };
    let picture = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let stats = drawpipe::Stats { edges: &mut [0; 4], off: &mut (0.0, 0.0) };
        owner.compose_owned(pipe, &mut Phosphor::new(w, h), &mut true, stats, None).frames
    }));
    if cfg!(debug_assertions) && matches!(loss, ComposeLoss::Overflow) {
        assert!(picture.is_err(), "the overflow guard trips after charging the loss");
    } else {
        assert_eq!(picture.unwrap(), pictured);
    }
    assert!(lock(&out).queue.is_empty());
    let fields = record_fields(&mut ui);
    assert_eq!(fields[field], lost, "{fields:?}");
    assert_conservation(&fields, pictured);
    fields
}

#[test]
fn state_reply_conserves_records_rejected_by_compose_key() {
    twice(|| compose_loss(ComposeLoss::StaleKey));
}

#[test]
fn state_reply_conserves_records_after_impossible_sixth_compose_input() {
    twice(|| compose_loss(ComposeLoss::Overflow));
}

#[test]
fn state_reply_conserves_records_lost_to_compose_error() {
    twice(|| compose_loss(ComposeLoss::Error));
}

#[test]
fn state_reply_conserves_records_filtered_by_a_software_zone() {
    let _scope = Scope::new();
    let out = HwShared::default();
    let mut ui = headless(&out, false);
    ui.m.zones[0] = crate::zone::Zone { on: true, pts: vec![(-1e-6, 100.0), (1e-6, 110.0)], ..Default::default() };
    crate::ui::change::settle(&mut ui);
    let want = lock(&out).want.clone().unwrap();
    {
        let mut g = lock(&out);
        let identity = g.acquisition.identity();
        g.record_flow.cost.observe(identity, 48, Duration::from_micros(48), Duration::ZERO);
    }
    let (mut io, _) = sim::fixture(&want, out.clone(), (Run::Selection, 48, false), None);
    let (mut source, _) = source(&out, &io);
    capture(&mut source, &mut io);
    let read = lock(&out).queue.len() as u64;
    assert_eq!(read, 48);
    ui.step(1.0 / 30.0);
    assert!(lock(&out).queue.is_empty());
    let fields = record_fields(&mut ui);
    assert_eq!(fields["read"], read);
    assert_conservation(&fields, 0);
    assert_eq!(fields["lost.settings"], read);
}

fn stop(ui: &mut Ui) {
    crate::scpi::Scpi::disconnected().exec_line(ui, &crate::Canvas::new(1, 1), ":STOP");
    ui.step(1.0 / 30.0);
    assert_eq!(ui.m.run, crate::model::RunState::Stop);
}
