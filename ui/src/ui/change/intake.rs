// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

struct Intake<'a>(&'a mut Ui);
impl Drop for Intake<'_> {
    fn drop(&mut self) {
        self.0.host.runtime_mut().change.timing.intake_end();
    }
}

pub(crate) fn intake<T>(ui: &mut Ui, f: impl FnOnce(&mut Ui) -> T) -> T {
    ui.host.runtime_mut().change.timing.intake_begin();
    let guard = Intake(ui);
    super::proposals::apply(guard.0);
    f(guard.0)
}

/// One lifecycle invocation, including its lock waits and any service work.
pub(in crate::ui) fn poll<T>(ui: &mut Ui, poll: crate::perfguard::deep::Poll, f: impl FnOnce(&mut Ui) -> T) -> T {
    let diagnostics = crate::perfguard::deep::counters();
    let wait = diagnostics.ui_wait();
    let started = std::time::Instant::now();
    let result = f(ui);
    let elapsed = started.elapsed();
    let wait = diagnostics.ui_wait().wrapping_sub(wait);
    let stopped = ui.memory_stopped();
    let observer = &mut ui.host.runtime_mut().change;
    if (observer.executing && !stopped) || observer.timing.intake_active() {
        observer.timing.poll(elapsed, wait);
    }
    diagnostics.poll(poll, elapsed);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perfguard::deep::{ui_lock, Poll};
    use std::time::Duration;

    fn parts(ui: &Ui) -> [u64; 5] {
        let text = ui.host.runtime().change.timing.diagnostics_text();
        let sample = text.split_once("diag intake.p50 n=1 ").unwrap().1;
        let mut values =
            sample.split_whitespace().take(5).map(|field| field.split_once('=').unwrap().1.parse().unwrap());
        std::array::from_fn(|_| values.next().unwrap())
    }

    #[test]
    fn completed_step_separates_nested_poll_external_body_and_shared_wait() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let mut ui = Ui::for_tests(Source::Hw { shared: Default::default(), seen: 0 });
        let shared = std::sync::Mutex::new(());
        std::thread::scope(|scope| {
            let (ready_tx, ready_rx) = std::sync::mpsc::channel();
            let (start_tx, start_rx) = std::sync::mpsc::channel();
            let shared = &shared;
            scope.spawn(move || {
                let _guard = oos_hal::sync::lock(shared);
                ready_tx.send(()).unwrap();
                start_rx.recv().unwrap();
                std::thread::sleep(Duration::from_millis(75));
            });
            ready_rx.recv().unwrap();
            intake(&mut ui, |ui| {
                intake(ui, |ui| {
                    poll(ui, Poll::Counter, |_| {
                        start_tx.send(()).unwrap();
                        let _guard = ui_lock(shared);
                        std::thread::sleep(Duration::from_millis(2));
                    });
                });
                std::thread::sleep(Duration::from_millis(2));
            });
        });
        ui.step(0.04);
        let [total, polls, external, wait, residual] = parts(&ui);
        assert_eq!(polls + external + wait + residual, total);
        assert!(wait >= 50_000_000, "wait={wait}");
        assert!(polls >= 2_000_000 && external >= 2_000_000);
        // Wait attribution cannot be manufactured by assigning all time to residual.
        assert!(wait > polls && wait > external, "polls={polls} external={external} wait={wait}");
    }

    #[test]
    fn untimed_poll_only_enters_call_ring() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let mut ui = Ui::for_tests(Source::Demo { t: 0.0 });
        poll(&mut ui, Poll::Counter, |_| {});
        ui.host.runtime_mut().change.timing.add(Duration::ZERO, Duration::ZERO);
        assert_eq!(parts(&ui), [0; 5]);
        assert!(crate::perfguard::deep::counters().text().contains("poll.counter n=1 total=1"));
    }
}
