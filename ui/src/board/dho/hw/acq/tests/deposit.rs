// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use drawpipe::test_scope::Submission;
use std::ops::Range;

fn submissions(result: &Outcome) -> Vec<&Submission> {
    result.events.iter().filter_map(|event| if let Event::Intake(batch) = event { Some(batch) } else { None }).collect()
}

fn assert_submission(actual: &Submission, range: Range<usize>, newest_seq: Option<u64>) {
    assert_eq!(actual.range(), range);
    let words: Vec<_> = range.map(|index| (index as u16) << 8).collect();
    assert_eq!(actual.first_words(), words);
    assert_eq!(actual.newest_seq(), newest_seq);
}

fn selected(
    (records, available): (u32, u32),
    (on_trigger, keep): (bool, bool),
    settings: Settings,
    expected: Option<Range<usize>>,
    queue: &[Frame],
) {
    let result = twice(|| run(Run::Selection, (records, available), settings.clone(), (on_trigger, keep), true));
    assert_eq!(result.decoded, u64::from(records));
    assert_eq!(result.queue, queue);
    let batches = submissions(&result);
    match expected {
        Some(range) => {
            let newest = (!range.is_empty()).then_some(u64::from(available));
            assert_eq!(batches.len(), 1);
            assert_submission(batches[0], range, newest);
            assert!(matches!(result.events.last(), Some(Event::Intake(_))));
        }
        None => assert!(batches.is_empty()),
    }
    assert_eq!(result.events.iter().filter(|event| matches!(event, Event::Dma(_))).count(), 1);
}

#[test]
fn initial_singleton_and_short_buffer_fall_back_unless_frame_zero_is_kept() {
    for records in [1, 4] {
        selected((records, 1), (false, false), settings(), None, &[]);
        selected((records, 1), (false, true), settings(), Some(0..1), &[]);
    }
}

#[test]
fn four_and_six_records_submit_the_kept_suffix_in_order() {
    for count in [4, 6] {
        selected((count, count), (true, false), settings(), Some(1..count as usize), &[]);
        selected((count, count), (false, false), settings(), Some(2..count as usize), &[]);
    }
}

#[test]
fn frame_one_edge_check_on_deposit_depends_on_coupling_and_trigger() {
    for (coupling, two, four) in
        [(tpu::Coupling::Dc, 1..1, 2..4), (tpu::Coupling::Ac, 1..2, 1..4), (tpu::Coupling::LfReject, 1..2, 1..4)]
    {
        let settings = Settings { trig_couple: coupling, ..settings() };
        selected((2, 2), (false, false), settings.clone(), Some(two), &[]);
        selected((4, 4), (false, false), settings, Some(four), &[]);
    }
    let non_edge = Settings { trigger: seq::Trig::Timeout { time: 1e-6 }, ..settings() };
    selected((2, 2), (false, false), non_edge.clone(), Some(1..2), &[]);
    selected((4, 4), (false, false), non_edge, Some(1..4), &[]);
}

#[test]
fn hardware_average_uses_record_selection_even_with_a_pipe() {
    let average = Settings { mode: Mode::Average { count: 2 }, ..settings() };
    selected((2, 2), (false, false), average.clone(), None, &[]);
    selected((2, 2), (false, true), average, None, &[Frame::average(2, 32770)]);
}

#[test]
fn deposit_wait_rechecks_after_virtual_timeout_and_resumes_on_pipe_wake() {
    let result = twice(|| run(Run::DepositBackpressure, (4, 4), settings(), (false, false), true));
    let batches = submissions(&result);
    assert_eq!(batches.len(), 2);
    assert_submission(batches[0], 2..4, Some(4));
    assert_submission(batches[1], 2..4, Some(8));
    assert_eq!(result.decoded, 8);
    assert!(result.queue.is_empty());

    let handoffs: Vec<_> = result
        .events
        .iter()
        .filter(|event| matches!(event, Event::Dma(_) | Event::ParkTimeout(_) | Event::Park(_) | Event::Intake(_)))
        .collect();
    assert!(matches!(
        handoffs.as_slice(),
        [Event::Dma(_), Event::Dma(_), Event::ParkTimeout(timeout), Event::Park(_), Event::Intake(_), Event::Intake(_)]
            if *timeout == Duration::from_millis(100)
    ));
}
