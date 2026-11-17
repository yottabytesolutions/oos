// SPDX-License-Identifier: GPL-2.0-or-later
//! Event waits for host tests. The timeout diagnoses a hang, never measures performance.
use std::time::Duration;

pub(crate) const HANG_GUARD: Duration = Duration::from_secs(120);

impl super::Ui {
    /// Await submitted jobs, then let the caller run the normal UI polls and state transitions.
    pub(crate) fn wait_background(&self) {
        self.wait_sim_event();
        self.wait_bus_event();
        self.wait_reference_measurement_event();
        self.wait_pair_measurement_event();
        self.wait_reference_decode_event();
        self.wait_reference_search_event();
        self.wait_math_event();
        self.wait_demo_event();
        self.wait_catalog_event();
        self.wait_eye_event();
        if let Some(worker) = &self.meas_bg {
            if worker.busy {
                worker.rx.wait_ready("background channel measurement completion");
            }
            if worker.analysis_busy {
                worker.analysis_rx.wait_ready("background full-record analysis completion");
            }
        }
    }
}

pub(crate) struct Wait {
    deadline: std::time::Instant,
    event: &'static str,
}
impl Wait {
    pub(crate) fn new(event: &'static str) -> Self {
        Self { deadline: std::time::Instant::now() + HANG_GUARD, event }
    }
    pub(crate) fn check(&self) {
        assert!(std::time::Instant::now() < self.deadline, "waiting for {}", self.event);
    }
    pub(crate) fn ui(&self, ui: &super::Ui) {
        self.check();
        ui.wait_background();
    }
}

impl Wait {
    /// Poll an observable completion counter when the worker has no blocking receiver.
    pub(crate) fn poll(&self) {
        self.check();
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub(crate) fn until<T>(event: &'static str, mut completed: impl FnMut() -> Option<T>) -> T {
    let wait = Wait::new(event);
    loop {
        if let Some(value) = completed() {
            return value;
        }
        wait.poll();
    }
}
