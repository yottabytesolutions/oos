// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use std::ops::Range;

mod fixture;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Signal {
    Off,
    Edge,
    Periodic,
    CoreOffsets,
}

struct Case {
    settings: Settings,
    count: u32,
    available: u32,
    keep: bool,
    signal: Signal,
    correct: bool,
    raw_edge: Option<bool>,
    average_hold: bool,
    held: bool,
}

impl Case {
    fn new(count: u32) -> Self {
        Self {
            settings: settings(),
            count,
            available: count,
            keep: false,
            signal: Signal::Off,
            correct: false,
            raw_edge: None,
            average_hold: false,
            held: false,
        }
    }
}

struct Expected {
    slots: Range<usize>,
    worker: bool,
    newest: Option<Frame>,
    losses: &'static [(&'static str, u64)],
}

impl Expected {
    fn queued(slots: Range<usize>, losses: &'static [(&'static str, u64)]) -> Self {
        let newest = (!slots.is_empty()).then(|| stamp(slots.end - 1));
        Self { slots, worker: false, newest, losses }
    }

    fn worker(slots: Range<usize>, losses: &'static [(&'static str, u64)]) -> Self {
        Self { worker: true, ..Self::queued(slots, losses) }
    }

    fn check(&self, result: &Outcome, sequence: u64, count: u32, available: u32) {
        let submissions = deposit::submissions(result);
        let pictures = result.events.iter().filter_map(|event| match event {
            Event::Picture(picture) => Some(picture),
            _ => None,
        });
        if self.worker && !self.slots.is_empty() {
            let [batch] = submissions.as_slice() else { panic!("expected one submission, got {submissions:?}") };
            assert_eq!(batch.range(), self.slots);
            assert_eq!(batch.first_words(), self.slots.clone().map(|slot| (slot as u16) << 8).collect::<Vec<_>>());
            assert_eq!(batch.newest_seq(), self.newest.as_ref().map(|_| sequence));
            assert!(result.queue.is_empty() && result.latest.is_none());
        } else {
            assert!(submissions.is_empty());
            let earlier: Vec<_> = self.slots.clone().take(self.slots.len().saturating_sub(1)).map(stamp).collect();
            let queue: Vec<_> = earlier.iter().chain(self.newest.as_ref()).collect();
            assert_eq!(result.queue.iter().collect::<Vec<_>>(), queue);
            assert_eq!(result.latest, self.newest);
        }
        for picture in pictures {
            assert_eq!(picture.frames, if self.worker { self.slots.len() as u64 } else { 0 });
            assert_eq!(picture.tap.as_ref().map(|tap| &tap.frame), self.newest.as_ref().filter(|_| self.worker));
        }
        self.check_counts(result, count, available);
    }

    fn check_counts(&self, result: &Outcome, count: u32, available: u32) {
        let deposited = if self.worker { self.slots.len() as u64 } else { 0 };
        assert_eq!((result.counters.read, result.counters.deposited), (u64::from(count), deposited));
        let held_skips = if result.final_ack == 0 { 0 } else { u64::from(available).saturating_sub(1) };
        assert_eq!(result.counters.dropped, held_skips);
        let mut actual: Vec<_> = result.counters.losses.iter().map(|(label, n)| (label.as_str(), *n)).collect();
        actual.sort_unstable();
        assert_eq!(actual, self.losses);
        assert_eq!(
            result.counters.read,
            result.counters.deposited + result.queue.len() as u64 + actual.iter().map(|(_, n)| n).sum::<u64>(),
            "every read slot must either survive or have one loss reason"
        );
        assert_eq!(result.decoded, u64::from(count));
        let skips = if result.final_ack != 0 {
            held_skips
        } else if self.worker || available == 0 {
            0
        } else {
            self.losses
                .iter()
                .filter(|(label, _)| matches!(*label, "lost.frame0" | "lost.frame1" | "lost.average"))
                .map(|(_, count)| count)
                .sum()
        };
        assert_eq!(result.dropped, skips);
        assert!(self.slots.end <= available as usize);
    }
}

fn stamp(slot: usize) -> Frame {
    Frame::stamp(slot as u64 + 1, slot as u16)
}

const FRAME0: &[(&str, u64)] = &[("lost.frame0", 1)];
const BOTH: &[(&str, u64)] = &[("lost.frame0", 1), ("lost.frame1", 1)];

fn check(case: Case, record: Expected, deposit: Expected) {
    let (records, worker) = twice(|| {
        let (records, record_seq, record_bytes) = fixture::trial(&case, false);
        let (worker, worker_seq, worker_bytes) = fixture::trial(&case, true);
        assert!(record_bytes == worker_bytes, "both routes must consume identical DMA bytes");
        ((records, record_seq), (worker, worker_seq))
    });
    let available = if case.average_hold { 0 } else { case.available };
    assert_eq!((records.1, worker.1), (u64::from(available), u64::from(available)));
    record.check(&records.0, records.1, case.count, available);
    deposit.check(&worker.0, worker.1, case.count, available);
}

#[test]
fn two_off_edge_records_never_submit_an_empty_batch() {
    check(Case::new(2), Expected::queued(1..1, BOTH), Expected::worker(1..1, BOTH));
}

#[test]
fn ready_two_record_average_is_published_off_edge_on_both_requests() {
    let mut case = Case::new(2);
    case.settings.mode = Mode::Average { count: 2 };
    let expected =
        || Expected { newest: Some(Frame::average(2, 32770)), ..Expected::queued(1..2, &[("lost.average", 1)]) };
    check(case, expected(), expected());
}

#[test]
fn both_routes_select_the_same_shallow_suffix() {
    for (count, signal, keep, first, losses) in [
        (1, Signal::Off, false, 1, &[("lost.settings", 1)][..]),
        (2, Signal::Edge, false, 1, FRAME0),
        (3, Signal::Edge, false, 1, FRAME0),
        (3, Signal::Off, false, 2, BOTH),
        (4, Signal::Edge, false, 1, FRAME0),
        (4, Signal::Off, false, 2, BOTH),
        (6, Signal::Edge, false, 1, FRAME0),
        (6, Signal::Off, false, 2, BOTH),
        (1, Signal::Off, true, 0, &[]),
        (2, Signal::Off, true, 0, &[]),
        (4, Signal::Off, true, 0, &[]),
    ] {
        let slots = first..count as usize;
        let record = Expected::queued(slots.clone(), losses);
        let deposit = if slots.is_empty() { Expected::queued(slots, losses) } else { Expected::worker(slots, losses) };
        check(Case { signal, keep, ..Case::new(count) }, record, deposit);
    }
}

#[test]
fn coupling_and_trigger_gate_the_same_frame_one_check() {
    for (coupling, trigger, first, losses) in [
        (tpu::Coupling::Dc, seq::Trig::Edge, 2, BOTH),
        (tpu::Coupling::Ac, seq::Trig::Edge, 1, FRAME0),
        (tpu::Coupling::LfReject, seq::Trig::Edge, 1, FRAME0),
        (tpu::Coupling::HfReject, seq::Trig::Edge, 2, BOTH),
        (tpu::Coupling::Dc, seq::Trig::Timeout { time: 1e-6 }, 1, FRAME0),
    ] {
        let mut case = Case::new(3);
        (case.settings.trig_couple, case.settings.trigger) = (coupling, trigger);
        check(case, Expected::queued(first..3, losses), Expected::worker(first..3, losses));
    }
}

#[test]
fn hardware_average_falls_back_to_decoded_records_with_or_without_a_pipe() {
    for (count, signal, keep, code) in
        [(2, Signal::Off, true, 32770), (2, Signal::Edge, false, 32770), (4, Signal::Off, false, 32778)]
    {
        let mut case = Case { signal, keep, ..Case::new(count) };
        case.settings.mode = Mode::Average { count: 2 };
        let expected = || Expected {
            newest: Some(Frame::average(u64::from(count), code)),
            ..Expected::queued(
                count as usize - 1..count as usize,
                if count == 2 { &[("lost.average", 1)] } else { &[("lost.average", 3)] },
            )
        };
        check(case, expected(), expected());
    }
}

#[test]
fn a_pending_hardware_average_consumes_no_sequences_on_either_request() {
    let mut case = Case { average_hold: true, ..Case::new(2) };
    case.settings.mode = Mode::Average { count: 4 };
    let expected = || Expected::queued(0..0, &[("lost.average", 2)]);
    check(case, expected(), expected());
}

#[test]
fn held_stop_keeps_the_newest_off_edge_slot_instead_of_running_selection() {
    let case = Case { held: true, ..Case::new(2) };
    let (result, seq) = twice(|| {
        let (outcome, seq, _) = fixture::trial(&case, true);
        (outcome, seq)
    });
    assert_eq!((seq, result.final_ack), (2, 7));
    Expected::queued(1..2, &[("lost.settings", 1)]).check(&result, seq, 2, 2);
}

#[test]
fn single_can_publish_the_first_record_with_frame_zero_drop() {
    let mut case = Case::new(1);
    case.settings.single = true;
    check(case, Expected::queued(0..1, &[]), Expected::worker(0..1, &[]));
}

#[test]
fn record_path_checks_deep_interior_frame_one_but_deposit_skips_it() {
    for (count, signal, first, losses) in
        [(2, Signal::Off, 1, BOTH), (3, Signal::Off, 2, BOTH), (3, Signal::Edge, 1, FRAME0)]
    {
        let mut case = Case { signal, ..Case::new(count) };
        (case.settings.depth, case.settings.tdiv) = (250_000, 100e-6);
        let worker = if count == 2 { Expected::worker(1..1, BOTH) } else { Expected::worker(1..3, FRAME0) };
        let end = if count == 2 { 1 } else { count as usize };
        check(case, Expected::queued(first..end, losses), worker);
    }
}

#[test]
fn record_path_rejects_corrected_frame_one_but_deposit_accepts_its_uncorrected_edge() {
    for (correct, first, losses) in [(false, 1, FRAME0), (true, 2, BOTH)] {
        let case = Case { signal: Signal::CoreOffsets, correct, ..Case::new(3) };
        check(case, Expected::queued(first..3, losses), Expected::worker(1..3, FRAME0));
    }
}

#[test]
fn record_path_checks_dma_frame_one_but_deposit_checks_the_separate_raw_newest() {
    let raw = || Some(Frame::stamp(2, fixture::RAW_SLOT_MARKER));
    let both_on = Case { signal: Signal::Edge, raw_edge: Some(true), ..Case::new(2) };
    check(
        both_on,
        Expected { newest: raw(), ..Expected::queued(1..2, FRAME0) },
        Expected { newest: raw(), ..Expected::worker(1..2, FRAME0) },
    );
    let dma_off = Case { raw_edge: Some(true), ..Case::new(2) };
    check(dma_off, Expected::queued(1..1, BOTH), Expected { newest: raw(), ..Expected::worker(1..2, FRAME0) });
    let raw_off = Case { signal: Signal::Edge, raw_edge: Some(false), ..Case::new(2) };
    check(raw_off, Expected { newest: raw(), ..Expected::queued(1..2, FRAME0) }, Expected::worker(1..1, BOTH));
}

#[test]
fn a_periodic_edge_in_every_window_passes_both_paths_without_proving_slot_age() {
    check(
        Case { signal: Signal::Periodic, ..Case::new(4) },
        Expected::queued(1..4, FRAME0),
        Expected::worker(1..4, FRAME0),
    );
}

#[test]
fn short_buffer_counts_missing_slots_and_skips_empty_handoffs() {
    for (available, slots, worker, losses) in [
        (0, 0..0, false, &[("lost.error", 4)][..]),
        (1, 1..1, false, &[("lost.error", 3), ("lost.settings", 1)]),
        (2, 1..1, true, &[("lost.error", 2), ("lost.frame0", 1), ("lost.frame1", 1)]),
        (3, 2..3, true, &[("lost.error", 1), ("lost.frame0", 1), ("lost.frame1", 1)]),
    ] {
        let case = Case { available, ..Case::new(4) };
        let record = Expected::queued(slots.clone(), losses);
        let deposit = if worker { Expected::worker(slots, losses) } else { Expected::queued(slots, losses) };
        check(case, record, deposit);
    }
}
