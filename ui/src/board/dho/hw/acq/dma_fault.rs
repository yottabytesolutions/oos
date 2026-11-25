// SPDX-License-Identifier: GPL-2.0-or-later

use super::{lifecycle, status, CaptureStatus, HwShared};
use crate::board::counters::Event;
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

#[derive(Default)]
pub(super) struct DmaFault {
    last_log: Option<Instant>,
    recovery: Option<Recovery>,
    #[cfg(feature = "private-picture-counts")]
    pub(super) inject_stall: bool,
}

struct Recovery {
    deadline: Instant,
    retry_at: Instant,
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
        if self.last_log.is_none_or(|last| now.saturating_duration_since(last) >= Duration::from_secs(5)) {
            let message = format!("{context}: {detail}");
            #[cfg(all(target_os = "linux", not(test)))]
            persist(&message);
            status(out, message);
            self.last_log = Some(now);
        }
        lifecycle::disarm(|| state.disarm_capture(bar, clock));
        self.recovery = Some(Recovery { deadline: now + LIMIT, retry_at: now + RETRY });
    }

    pub(super) fn pending(&self) -> bool {
        self.recovery.is_some()
    }

    pub(super) fn recover(
        &mut self,
        state: &mut seq::State,
        bar: &mut impl Bus,
        clock: &impl Clock,
        out: &HwShared,
    ) -> bool {
        let Some(recovery) = self.recovery.as_mut() else { return true };
        let now = clock.now();
        let fault = match bar.try_rd(0x4034) {
            Ok(u32::MAX) => Some((CaptureStatus::DmaStopLink, "0x4034 returned all ones".into())),
            Err(error) => Some((CaptureStatus::DmaStopLink, format!("0x4034: {error}"))),
            Ok(value) if value & scu::status::STOPPED != 0 => {
                self.recovery = None;
                return true;
            }
            Ok(value) if now >= recovery.deadline => {
                Some((CaptureStatus::DmaStopDeadline, format!("0x4034={value:#010x}")))
            }
            Ok(_) => None,
        };
        if let Some((fault, detail)) = fault {
            self.recovery = None;
            let message = {
                let mut g = crate::perfguard::deep::acq_lock(out);
                let publication = g.acquisition;
                g.status.report(publication, fault, Some(&detail))
            };
            eprintln!("oos-ui hw: {message}");
            return true;
        }
        if now >= recovery.retry_at {
            lifecycle::disarm(|| state.disarm_capture(bar, clock));
            while recovery.retry_at <= now {
                recovery.retry_at += RETRY;
            }
        }
        let now = clock.now();
        let next = recovery.retry_at.min(recovery.deadline).saturating_duration_since(now);
        clock.wait(POLL.min(next));
        false
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
mod tests;
