// SPDX-License-Identifier: GPL-2.0-or-later

use super::{lifecycle, status, CaptureStatus, HwShared};
use crate::{board::counters::Event, perfguard::deep::acq_lock as lock};
use oos_hal::dho::{
    bus::Bus,
    scu,
    seq::{
        self,
        time::{Clock, ExecuteError},
    },
};
use std::time::{Duration, Instant};

const POLL: Duration = Duration::from_millis(20);
const RETRY: Duration = Duration::from_millis(200);
const LIMIT: Duration = Duration::from_secs(1);
const BACKOFF_CAP: Duration = Duration::from_secs(30);
const LOG_PERIOD: Duration = Duration::from_secs(5);

#[derive(Default)]
pub(super) struct DmaFault {
    last_log: Option<Instant>,
    last_recovery_log: Option<Instant>,
    backoff: Duration,
    recovery: Option<Recovery>,
    #[cfg(feature = "private-picture-counts")]
    pub(super) inject_stall: bool,
}

#[derive(Clone, Copy)]
enum Recovery {
    Stopping { deadline: Instant, retry_at: Instant },
    Backoff { until: Instant },
}

impl DmaFault {
    pub(super) fn report(
        &mut self,
        state: &mut seq::State,
        bar: &mut impl Bus,
        clock: &impl Clock,
        out: &HwShared,
        context: &str,
        error: ExecuteError,
    ) {
        let ExecuteError::DmaTimeout(detail) = error else {
            status(out, format!("{context}: {error}"));
            return;
        };
        crate::drawpipe::counters().acquisition.add(Event::DmaTimeout);
        let now = clock.now();
        if log_due(&mut self.last_log, now) {
            let message = format!("{context}: {detail}");
            #[cfg(all(target_os = "linux", not(test)))]
            persist(&message);
            status(out, message);
        }
        self.start_stop(state, bar, clock);
    }

    pub(super) fn pending(&self) -> bool {
        self.recovery.is_some()
    }

    pub(super) fn read_succeeded(&mut self) {
        self.backoff = Duration::ZERO;
    }

    fn start_stop(&mut self, state: &mut seq::State, bar: &mut impl Bus, clock: &impl Clock) {
        let now = clock.now();
        lifecycle::disarm(|| state.disarm_capture(bar, clock));
        self.recovery = Some(Recovery::Stopping { deadline: now + LIMIT, retry_at: now + RETRY });
    }

    pub(super) fn recover(
        &mut self,
        state: &mut seq::State,
        bar: &mut impl Bus,
        clock: &impl Clock,
        out: &HwShared,
    ) -> bool {
        let now = clock.now();
        let (deadline, mut retry_at) = match self.recovery {
            None => return true,
            Some(Recovery::Stopping { deadline, retry_at }) => (deadline, retry_at),
            Some(Recovery::Backoff { until }) => {
                if now < until {
                    clock.wait(POLL.min(until - now));
                } else {
                    lock(out).status.set_dma_backoff(None);
                    self.start_stop(state, bar, clock);
                }
                return false;
            }
        };
        let fault = match bar.try_rd(0x4034) {
            Ok(u32::MAX) => Some((CaptureStatus::DmaStopLink, "0x4034 returned all ones".into())),
            Err(error) => Some((CaptureStatus::DmaStopLink, format!("0x4034: {error}"))),
            Ok(value) if value & scu::status::STOPPED != 0 => {
                self.recovery = None;
                return true;
            }
            Ok(value) if now >= deadline => Some((CaptureStatus::DmaStopDeadline, format!("0x4034={value:#010x}"))),
            Ok(_) => None,
        };
        if let Some((fault, detail)) = fault {
            self.back_off(out, now, fault, &detail);
            return false;
        }
        if now >= retry_at {
            lifecycle::disarm(|| state.disarm_capture(bar, clock));
            while retry_at <= now {
                retry_at += RETRY;
            }
            self.recovery = Some(Recovery::Stopping { deadline, retry_at });
        }
        let now = clock.now();
        let next = retry_at.min(deadline).saturating_duration_since(now);
        clock.wait(POLL.min(next));
        false
    }

    fn back_off(&mut self, out: &HwShared, now: Instant, fault: CaptureStatus, detail: &str) {
        self.backoff = (self.backoff * 2).clamp(LIMIT, BACKOFF_CAP);
        self.recovery = Some(Recovery::Backoff { until: now + self.backoff });
        let message = {
            let mut g = lock(out);
            g.status.set_dma_backoff(Some(now));
            let publication = g.acquisition;
            log_due(&mut self.last_recovery_log, now).then(|| g.status.report(publication, fault, Some(detail)))
        };
        if let Some(message) = message {
            eprintln!("oos-ui hw: {message}");
        }
    }
}

fn log_due(last: &mut Option<Instant>, now: Instant) -> bool {
    if last.is_some_and(|last| now.saturating_duration_since(last) < LOG_PERIOD) {
        return false;
    }
    *last = Some(now);
    true
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
mod tests;
