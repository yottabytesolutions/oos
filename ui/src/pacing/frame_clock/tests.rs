// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const REFRESH: Duration = Duration::from_nanos(16_666_667);
const PANEL_PIXELS: u64 = 1084 * 644;
const PANEL_CLOCK: u64 = 68_000_000;
const DIVISORS: [FlipDivisor; 3] = [FlipDivisor::Two, FlipDivisor::Three, FlipDivisor::Four];

fn panel_refresh() -> Duration {
    Duration::from_secs_f64(PANEL_PIXELS as f64 / PANEL_CLOCK as f64)
}

fn panel_flip(start: Instant, sequence: u32) -> Flip {
    // DRM completion timestamps have microsecond precision.
    let micros = u64::from(sequence) * PANEL_PIXELS * 1_000_000 / PANEL_CLOCK;
    Flip { sequence, at: start + Duration::from_micros(micros) }
}

fn flip(clock: &mut FrameClock, origin: Instant, sequence: u32, delay: Duration) -> (Option<Duration>, bool) {
    let at = origin + REFRESH * sequence;
    clock.advance(at + delay, Some(Flip { sequence, at }))
}

#[test]
fn timer_uses_picture_period_and_ignores_flip_completions() {
    let at = Instant::now();
    let mut clock = FrameClock::new(ClockMode::Timer, FlipDivisor::Three, at, panel_refresh());
    assert_eq!(flip(&mut clock, at, 1, Duration::ZERO), (None, false));
    assert_eq!(clock.deadline(at), at + TIMER_PERIOD);
    assert_eq!(clock.advance(at + TIMER_PERIOD, None), (Some(TIMER_PERIOD), false));
    assert_eq!(clock.wait_until(), at + TIMER_PERIOD * 2);
}

#[test]
fn panel_steps_and_measured_rate_follow_each_divisor_without_drift() {
    for (divisor, expected_fps) in DIVISORS.into_iter().zip(["48.7", "32.5", "24.4"]) {
        let start = Instant::now();
        let mut clock = FrameClock::new(ClockMode::Flip, divisor, start, panel_refresh());
        let mut rate = PictureRate::default();
        let mut reported = None;
        let divisor = divisor as u32;
        for seq in 1..=480 {
            let event = panel_flip(start, seq);
            let (elapsed, fallback) = clock.advance(event.at, Some(event));
            assert!(!fallback);
            assert_eq!(elapsed.is_some(), seq.is_multiple_of(divisor), "divisor {divisor}, flip {seq}");
            if elapsed.is_some() {
                reported = rate.observe(event.at).or(reported);
                clock.step_finished(event.at + Duration::from_micros(1));
            }
            let next = panel_flip(start, seq + divisor - seq % divisor).at;
            assert!(
                clock.deadline(event.at).max(next) - clock.deadline(event.at).min(next) <= Duration::from_micros(1)
            );
        }
        assert_eq!(format!("{:.1}", reported.unwrap()), expected_fps);
        assert_eq!(clock.skipped(), 0);
    }
}

#[test]
fn regular_completions_start_steps_on_even_sequences_only() {
    let at = Instant::now();
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, at, REFRESH);
    for seq in 1..=6 {
        let now = at + REFRESH * seq;
        assert_eq!(flip(&mut clock, at, seq, Duration::ZERO).0.is_some(), seq % 2 == 0);
        if seq % 2 == 0 {
            clock.step_finished(now);
        }
        let next_even = seq + if seq % 2 == 0 { 2 } else { 1 };
        assert_eq!(clock.deadline(now), at + REFRESH * next_even);
    }
}

#[test]
fn duplicate_sequence_cannot_start_a_second_step() {
    let at = Instant::now();
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, at, REFRESH);
    assert!(flip(&mut clock, at, 2, Duration::ZERO).0.is_some());
    assert_eq!(flip(&mut clock, at, 2, Duration::from_millis(1)), (None, false));
}

#[test]
fn publication_predicts_an_immediate_step_while_a_slot_is_busy() {
    let start = Instant::now();
    let refresh = Duration::from_millis(10);
    for busy in [FrameSlot::HelperBusy, FrameSlot::PanelBusy] {
        let published = ClockPublication::new(start);
        let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, start, refresh);
        for (ms, sequence, slot, elapsed, deadline) in [
            (10, Some(1), FrameSlot::Ready, None, 20),
            (20, Some(2), busy, None, 20),
            (22, None, busy, None, 22),
            (23, None, FrameSlot::Ready, Some(23), 40),
            (24, None, FrameSlot::Ready, None, 40),
        ] {
            let now = start + Duration::from_millis(ms);
            let edge = sequence.map(|sequence| Flip { sequence, at: now });
            assert_eq!(clock.advance_available(now, edge, slot), (elapsed.map(Duration::from_millis), false));
            published.publish(&clock, now);
            assert_eq!(
                published.next_step_after(now),
                Some(start + Duration::from_millis(deadline)),
                "{busy:?} at {ms} ms"
            );
        }
        assert_eq!(published.skipped(), 0);
    }
}

#[test]
fn a_new_due_step_supersedes_only_one_pending_step() {
    let start = Instant::now();
    for (slot, helper_skipped) in [(FrameSlot::HelperBusy, 1), (FrameSlot::PanelBusy, 0)] {
        let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, start, REFRESH);
        for sequence in [2, 4] {
            let at = start + REFRESH * sequence;
            assert_eq!(clock.advance_available(at, Some(Flip { sequence, at }), slot), (None, false));
        }
        let ready = start + REFRESH * 4 + Duration::from_millis(1);
        assert_eq!(clock.advance(ready, None), (Some(ready - start), false));
        assert_eq!(clock.advance(ready, None), (None, false));
        assert_eq!((clock.skipped(), clock.helper_skipped()), (1, helper_skipped));
    }
}

#[test]
fn one_missing_completion_keeps_the_hardware_phase() {
    let at = Instant::now();
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, at, REFRESH);
    for seq in [1, 2, 4, 5, 7, 8] {
        assert_eq!(
            flip(&mut clock, at, seq, Duration::ZERO),
            ((seq % 2 == 0).then_some(REFRESH * if seq == 8 { 4 } else { 2 }), false)
        );
    }
}

#[test]
fn overrun_discards_a_completion_during_the_previous_step() {
    let at = Instant::now();
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, at, REFRESH);
    assert!(flip(&mut clock, at, 2, Duration::ZERO).0.is_some());
    clock.step_finished(at + REFRESH * 4 + Duration::from_millis(1));
    assert_eq!(flip(&mut clock, at, 4, Duration::from_millis(1)), (None, false));
    assert_eq!(flip(&mut clock, at, 5, Duration::ZERO), (None, false));
    assert_eq!(flip(&mut clock, at, 6, Duration::ZERO), (Some(REFRESH * 4), false));
    assert_eq!(clock.skipped(), 1);
    let published = ClockPublication::new(at);
    published.publish(&clock, at + REFRESH * 6);
    assert_eq!(published.skipped(), 1);
}

#[test]
fn a_flip_during_post_presentation_work_still_starts_the_next_step() {
    let start = Instant::now();
    let refresh = Duration::from_millis(10);
    let duration = |ms| Duration::from_millis(ms);
    let event = |sequence| Flip { sequence, at: start + refresh * sequence };
    let handed_off = start + duration(35);
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, start, refresh);
    assert_eq!(clock.advance(event(3).at, Some(event(3))).0, Some(duration(30)));
    let mut whole_loop = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, start, refresh);
    whole_loop.advance(event(3).at, Some(event(3)));
    whole_loop.step_finished(start + duration(62));
    clock.step_finished(start + duration(55));

    // The due flip landed during housekeeping and was observed 2 ms later.
    assert_eq!(clock.advance(start + duration(62), Some(event(6))).0, Some(duration(32)));
    assert_eq!(whole_loop.advance(start + duration(62), Some(event(6))).0, None);
    assert!(whole_loop.advance(event(9).at, Some(event(9))).0.is_some());
    assert_eq!(event(9).at - handed_off, duration(55));
    assert_eq!(start + duration(62) - handed_off, duration(27));
    clock.step_finished(start + duration(70));
    assert_eq!(clock.advance(event(7).at, Some(event(7))).0, None);
    assert_eq!(clock.advance(event(9).at, Some(event(9))).0, Some(duration(28)));
}

#[test]
fn a_completion_a_whole_period_late_does_not_start_a_catch_up_step() {
    let start = Instant::now();
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, start, REFRESH);
    assert!(flip(&mut clock, start, 2, Duration::ZERO).0.is_some());
    clock.step_finished(start + REFRESH * 3);
    assert_eq!(flip(&mut clock, start, 4, REFRESH * 2), (None, false));
    assert_eq!(flip(&mut clock, start, 7, Duration::ZERO), (None, false));
    assert_eq!(flip(&mut clock, start, 8, Duration::ZERO).0, Some(REFRESH * 6));
}

#[test]
fn each_panel_divisor_keeps_its_period_during_fallback_and_recovers_without_a_double_step() {
    for divisor in DIVISORS {
        let start = Instant::now();
        let published = ClockPublication::new(start);
        let mut clock = FrameClock::new(ClockMode::Flip, divisor, start, panel_refresh());
        let seq = divisor as u32;
        let first = panel_flip(start, seq);
        assert!(clock.advance(first.at, Some(first)).0.is_some());
        clock.step_finished(first.at);
        let period = clock.period();
        let timeout = first.at + FLIP_TIMEOUT;
        assert_eq!(clock.advance(timeout - Duration::from_nanos(1), None), (None, false));
        assert_eq!(clock.advance(timeout, None), (Some(FLIP_TIMEOUT), true));
        published.publish(&clock, timeout);
        assert_eq!(published.period(), period);
        let timed = timeout + period;
        assert_eq!(published.next_step_after(timed), Some(timed));
        assert_eq!(published.next_step_after(timed + Duration::from_nanos(1)), Some(timed + period));
        assert_eq!(clock.advance(timed - Duration::from_nanos(1), None), (None, false));
        assert_eq!(clock.advance(timed, None), (Some(period), false));
        clock.step_finished(timed);

        let first_return = (seq + 1..).find(|&n| panel_flip(start, n).at > timed).unwrap();
        let mut steps = 0;
        for n in first_return..first_return + seq * 2 {
            let event = panel_flip(start, n);
            let (elapsed, fallback) = clock.advance(event.at, Some(event));
            assert!(!fallback);
            assert_eq!(elapsed.is_some(), n.is_multiple_of(seq) && event.at >= timed + period);
            if elapsed.is_some() {
                clock.step_finished(event.at);
                steps += 1;
            }
        }
        assert_eq!(steps, 1);
        let missed = u64::from(divisor == FlipDivisor::Two);
        assert_eq!(clock.skipped(), missed);
        assert_eq!(published.skipped(), missed);
        published.publish(&clock, panel_flip(start, first_return + 2 * seq - 1).at);
        assert_eq!(published.period(), period);
    }
}

#[test]
fn a_flip_read_after_a_60_ms_step_does_not_trigger_fallback() {
    let at = Instant::now();
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, at, REFRESH);
    assert_eq!(flip(&mut clock, at, 2, Duration::ZERO), (Some(REFRESH * 2), false));
    let finished = at + REFRESH * 2 + Duration::from_millis(60);
    clock.step_finished(finished);
    // The outstanding flip completed on the next refresh, while the step was still running.
    let read = finished + Duration::from_millis(9);
    assert_eq!(clock.advance(read, Some(Flip { sequence: 3, at: at + REFRESH * 3 })), (None, false));
    assert_eq!(clock.wait_until(), read + FLIP_TIMEOUT);
    assert_eq!(flip(&mut clock, at, 7, Duration::ZERO), (None, false));
    assert_eq!(flip(&mut clock, at, 8, Duration::ZERO), (Some(REFRESH * 6), false));
    assert_eq!(clock.skipped(), 2);
}

#[test]
fn timer_and_flip_fallback_count_elapsed_whole_periods() {
    let start = Instant::now();
    for mode in [ClockMode::Timer, ClockMode::Flip] {
        let mut clock = FrameClock::new(mode, FlipDivisor::Two, start, REFRESH);
        let period = clock.period();
        let first = start + period * 3 + period / 2;
        assert_eq!(clock.advance(first, None).0, Some(first - start));
        assert_eq!(clock.skipped(), 2);
        assert_eq!(clock.advance(first + period - Duration::from_nanos(1), None).0, None);
        assert_eq!(clock.advance(first + period, None).0, Some(period));
        assert_eq!(clock.skipped(), 2);
    }
}

#[test]
fn fallback_does_not_recount_a_missed_flip_since_the_last_step() {
    let start = Instant::now();
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Two, start, REFRESH);
    assert!(flip(&mut clock, start, 2, Duration::ZERO).0.is_some());
    assert_eq!(flip(&mut clock, start, 5, Duration::ZERO).0, None);
    assert_eq!(clock.skipped(), 1);
    assert!(clock.advance(start + REFRESH * 10, None).0.is_some());
    assert_eq!(clock.skipped(), 3);
}

#[test]
fn a_three_refresh_cadence_keeps_its_phase_across_sequence_wrap() {
    let start = Instant::now();
    let refresh = Duration::from_millis(12);
    let mut clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, start, refresh);
    for (index, sequence) in [u32::MAX - 1, u32::MAX, 0, 1, 2, 3, 4, 5].into_iter().enumerate() {
        let at = start + refresh * (index as u32 + 1);
        let elapsed = clock.advance(at, Some(Flip { sequence, at })).0;
        assert_eq!(elapsed.is_some(), matches!(sequence, u32::MAX | 2 | 5));
    }
    assert_eq!(clock.skipped(), 0);
}

#[test]
fn panel_periods_derive_pipeline_and_work_budgets() {
    use crate::drawpipe::budget::{default_work_budget, pipeline_budget};

    let start = Instant::now();
    let timings = [
        [20_532_236, 15_399_177, 7_391_605],
        [30_798_354, 23_098_766, 11_087_407],
        [41_064_472, 30_798_354, 14_783_210],
    ];
    for (divisor, expected) in DIVISORS.into_iter().zip(timings) {
        let period = FrameClock::new(ClockMode::Flip, divisor, start, panel_refresh()).period();
        assert_eq!([period, pipeline_budget(period), default_work_budget(period)], expected.map(Duration::from_nanos));
    }
    assert_eq!([pipeline_budget(TIMER_PERIOD), default_work_budget(TIMER_PERIOD)], [25, 12].map(Duration::from_millis));
}

#[test]
fn measured_rate_counts_missing_steps_as_elapsed_time() {
    let start = Instant::now();
    let mut rate = PictureRate::default();
    assert_eq!(rate.observe(start), None);
    assert_eq!(rate.observe(start + Duration::from_millis(600)), None);
    assert_eq!(rate.observe(start + Duration::from_secs(1)), Some(2.0));
    assert_eq!(rate.observe(start + Duration::from_secs(2)), Some(1.0));
}
