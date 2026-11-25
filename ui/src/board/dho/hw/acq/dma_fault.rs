// SPDX-License-Identifier: GPL-2.0-or-later

use super::{disarm, status, HwShared};
use crate::board::counters::Event;
use oos_hal::dho::{
    bus::Bus,
    scu,
    seq::{
        self,
        time::{Clock, DMA_TIMEOUT},
    },
};
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct DmaFault {
    last_log: Option<Instant>,
    awaiting_idle: bool,
    #[cfg(all(feature = "private-picture-counts", debug_assertions))]
    pub(super) inject_stall: bool,
}

pub(super) fn is_timeout(message: &str) -> bool {
    message.contains(DMA_TIMEOUT)
}

impl DmaFault {
    pub(super) fn report(
        &mut self,
        state: &mut seq::State,
        bar: &mut impl Bus,
        clock: &impl Clock,
        out: &HwShared,
        message: String,
    ) {
        if !is_timeout(&message) {
            status(out, message);
            return;
        }
        crate::drawpipe::counters().acquisition.add(Event::DmaTimeout);
        let now = clock.now();
        if self.last_log.is_none_or(|last| now.saturating_duration_since(last) >= Duration::from_secs(5)) {
            #[cfg(all(target_os = "linux", not(test)))]
            persist(&message);
            status(out, message);
            self.last_log = Some(now);
        }
        disarm(state, bar, clock, out);
        self.awaiting_idle = true;
    }

    pub(super) fn pending(&self) -> bool {
        self.awaiting_idle
    }

    pub(super) fn ready(&mut self, bar: &mut impl Bus) -> bool {
        if self.awaiting_idle && bar.try_rd(0x4034).is_ok_and(|v| v != u32::MAX && v & scu::status::STOPPED != 0) {
            self.awaiting_idle = false;
        }
        !self.awaiting_idle
    }
}

#[cfg(all(target_os = "linux", not(test)))]
fn persist(message: &str) {
    use std::io::Write as _;
    // A single pmsg record survives warm reboot in ramoops alongside kernel diagnostics.
    let line = format!("oos-ui hw: {message}\n");
    if let Err(error) =
        std::fs::OpenOptions::new().write(true).open("/dev/pmsg0").and_then(|mut file| file.write_all(line.as_bytes()))
    {
        eprintln!("oos-ui hw: cannot save DMA timeout to pstore: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawpipe::{self, test_scope::Scope};
    use oos_hal::{dho::bus::Recorder, sync::lock};
    use std::cell::Cell;

    struct Virtual(Cell<Instant>);

    impl Clock for Virtual {
        fn now(&self) -> Instant {
            self.0.get()
        }

        fn wait(&self, duration: Duration) {
            self.0.set(self.now() + duration);
        }
    }

    #[test]
    fn counts_every_timeout_and_waits_for_valid_stopped_status() {
        let _scope = Scope::new();
        let clock = Virtual(Cell::new(Instant::now()));
        let mut fault = DmaFault::default();
        let mut state = seq::State::handover(1, None);
        let mut bar = Recorder::default();
        let out = HwShared::default();
        bar.regs.insert(0x4034, scu::status::STOPPED);

        fault.report(&mut state, &mut bar, &clock, &out, format!("{DMA_TIMEOUT}: first"));
        assert_eq!(lock(&out).status.take_latest(), format!("{DMA_TIMEOUT}: first"));
        bar.regs.insert(0x4034, u32::MAX);
        assert!(!fault.ready(&mut bar));
        bar.regs.insert(0x4034, 0);
        assert!(!fault.ready(&mut bar));
        bar.regs.insert(0x4034, scu::status::STOPPED);
        assert!(fault.ready(&mut bar));

        clock.wait(Duration::from_secs(1));
        fault.report(&mut state, &mut bar, &clock, &out, format!("{DMA_TIMEOUT}: second"));
        assert!(lock(&out).status.take_latest().is_empty());
        clock.wait(Duration::from_secs(4));
        fault.report(&mut state, &mut bar, &clock, &out, format!("{DMA_TIMEOUT}: third"));
        assert_eq!(lock(&out).status.take_latest(), format!("{DMA_TIMEOUT}: third"));
        assert!(drawpipe::counters().acquisition.text().contains(" acq_dma_timeouts=3"));
    }
}
