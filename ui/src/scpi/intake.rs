// SPDX-License-Identifier: GPL-2.0-or-later
//! Frame-owned FIFO execution, continuation ownership and parked reply delivery.
use super::Program;
use crate::netport::{Envelope, Ports, RemoteRequest, Reply, Reservation};
use std::time::Duration;

pub(super) struct Transport {
    pub(super) endpoint: crate::netio::Endpoint,
    pub(super) intake: Intake,
    pub(super) reply_limit: usize,
}
impl Default for Transport {
    fn default() -> Self {
        Self { endpoint: Default::default(), intake: Default::default(), reply_limit: super::RESP_MAX }
    }
}

#[derive(Default)]
pub(super) struct Intake {
    active: Option<Active>,
    parked: Option<Reply>,
}
struct Active {
    request: Envelope,
    program: Program,
    reservation: Reservation,
}

impl Intake {
    pub(super) fn poll(
        &mut self,
        ports: &Ports,
        mut elapsed: impl FnMut() -> Duration,
        mut execute: impl FnMut(&mut Program) -> Option<Vec<u8>>,
    ) {
        for _ in 0..8 {
            if elapsed() >= Duration::from_micros(500) {
                ports.notify_frame();
                break;
            }
            if !ports.flush(&mut self.parked) {
                return;
            }
            if self.active.is_none() {
                if ports.requests().is_empty() {
                    break;
                }
                let Some(reservation) = ports.budget().reserve_available() else { break };
                let Some(request) = ports.requests().take(1).pop() else { break };
                let RemoteRequest::Program(text) = request.request() else { continue };
                let mut program = Program::new(text);
                program.output = Some(crate::netport::Output::bounded(reservation.capacity()));
                self.active = Some(Active { request, program, reservation });
            }
            let Some(mut active) = self.active.take() else { break };
            if active.request.cancelled() {
                continue;
            }
            if active.program.waiting.is_some()
                && active.request.read_closed().load(std::sync::atomic::Ordering::Acquire)
            {
                self.parked = Some(Reply::new(active.request.id(), Vec::new(), active.reservation));
                continue;
            }
            let Some(mut bytes) = execute(&mut active.program) else {
                if active.request.read_closed().load(std::sync::atomic::Ordering::Acquire) {
                    self.parked = Some(Reply::new(active.request.id(), Vec::new(), active.reservation));
                    continue;
                }
                self.active = Some(active);
                break;
            };
            let mut output = active.program.output.take().unwrap_or_default();
            if !bytes.is_empty() {
                let _ = output.push(std::mem::take(&mut bytes));
            }
            if active.program.has_reply {
                let _ = output.push(vec![b'\n']);
            }
            self.parked = Some(Reply::from_output(active.request.id(), output, active.reservation));
        }
        // A completion at the quota boundary still has to reach IO on a later intake.
        if self.parked.is_some() {
            ports.notify_frame();
        }
    }
    #[cfg(test)]
    pub(super) fn active_is_empty(&self) -> bool {
        self.active.is_none()
    }
    #[cfg(test)]
    pub(super) fn pending(&self) -> bool {
        self.active.as_ref().is_some_and(|a| a.program.waiting.is_some())
    }
    #[cfg(test)]
    pub(super) fn cancelled(&self) -> bool {
        self.active
            .as_ref()
            .is_none_or(|a| a.request.cancelled() || a.request.read_closed().load(std::sync::atomic::Ordering::Acquire))
    }
}

#[cfg(test)]
mod tests;

impl super::Scpi {
    pub fn io_runtime(&mut self) -> std::io::Result<crate::netio::Runtime> {
        self.transport.endpoint.runtime()
    }
    pub fn set_open(&mut self, port: Option<u16>, v6: bool) {
        self.transport.endpoint.set_open(port.map(Into::into), v6, crate::netio::Protocol::Raw);
        if port.is_none() {
            self.transport.intake = Default::default();
        }
    }
}
