// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::unit_tools::scpi::TimedReply;

struct PacedScpi<'a> {
    inner: FakeScpi<'a>,
    now: Rc<Cell<Duration>>,
    initial_pace: Duration,
    latency: Duration,
}

impl Scpi for PacedScpi<'_> {
    fn send(&mut self, command: &str) -> Result<(), String> {
        self.inner.send(command)
    }

    fn query_timed<T>(&mut self, command: &str, mut now: impl FnMut() -> T) -> Result<TimedReply<T>, String> {
        self.now.set(self.now.get() + std::mem::take(&mut self.initial_pace));
        let before_write = now();
        let line = self.inner.query(command)?;
        self.now.set(self.now.get() + self.latency);
        Ok(TimedReply { line, before_write, after_line: now() })
    }

    fn reconnect(&mut self) -> Result<(), String> {
        self.inner.reconnect()
    }
}

#[test]
fn pacing_before_the_first_write_does_not_widen_window_bounds() {
    for initial_pace in [Duration::ZERO, Duration::from_millis(150)] {
        let events = Events::default();
        let mut script = Script::default();
        script.reply(STATE, BEFORE);
        script.reply(STATE, AFTER);
        let mut clock = FakeClock::default();
        let mut scpi = PacedScpi {
            inner: script.scpi(&events),
            now: Rc::clone(&clock.now),
            initial_pace,
            latency: Duration::from_millis(2),
        };
        let window = window::measure(&mut scpi, &mut clock, Duration::from_secs(5)).unwrap();
        assert_eq!(clock.waits, [Duration::from_secs(5)]);
        assert_eq!(window.start_s, initial_pace.as_secs_f64());
        assert_eq!(window.host_s, [5.0, 5.004]);
        let WindowSample::Valid(sample) = window.sample else { panic!("expected valid window") };
        let midpoint = f64::midpoint(5.0, 5.004);
        assert_eq!(sample.rates.fresh_fps, [30.0 / 5.004, 6.0]);
        assert_eq!(sample.rates.shown_records_per_s, 110.0 / midpoint);
        assert_eq!(sample.rates.first_records_per_s, 100.0 / midpoint);
        assert_eq!(sample.rates.read_per_s, 130.0 / midpoint);
        assert_eq!(sample.rates.late_flips, 2);
        assert_eq!(sample.fallback_delta, [("decoded_lines".into(), 3)].into());
        assert!(scpi.inner.replies.is_empty());
    }
}

#[test]
fn checks_loss_route_and_fallback_counters_but_allows_gauges_to_decrease() {
    for fields in
        ["source.v1 parallelism=4", "lost.queue_overflow=4", "route.v1 dense=4", "fallback.v1 decoded_lines=4"]
    {
        let events = Events::default();
        let mut script = Script::default();
        script.reply(STATE, &format!("{BEFORE} stage.v1 {fields}"));
        script.reply(STATE, &format!("{AFTER} stage.v1 {}", fields.replace('4', "3")));
        let mut scpi = script.scpi(&events);
        let window = window::measure(&mut scpi, &mut FakeClock::default(), Duration::from_secs(5)).unwrap();
        if fields.contains("parallelism") {
            let WindowSample::Valid(sample) = window.sample else { panic!("expected valid window") };
            assert_eq!(sample.sections["source.v1"]["parallelism"], "3");
        } else {
            let encoded = serde_json::to_value(&window).unwrap();
            assert_eq!(encoded["sample"], "counter reset");
            for field in ["delta", "fallback_delta", "rates", "sections"] {
                assert!(encoded.get(field).is_none());
            }
        }
    }
}
