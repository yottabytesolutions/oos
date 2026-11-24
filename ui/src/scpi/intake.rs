// SPDX-License-Identifier: GPL-2.0-or-later
//! Frame-owned execution with one continuation per waiting connection.
use super::{Canvas, Program, Ui};
use crate::netport::{Envelope, Ports, RemoteRequest, Reply, Reservation};
use std::{collections::VecDeque, sync::atomic::Ordering, time::Duration};

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
    waiting: VecDeque<Active>,
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
        mut execute: impl FnMut(&mut Program, &mut Reservation) -> Option<Vec<u8>>,
    ) {
        let waiting = self.waiting.len();
        for turn in 0..crate::netport::MAX_CLIENTS {
            if elapsed() >= Duration::from_micros(500) {
                ports.notify_frame();
                break;
            }
            if !ports.flush(&mut self.parked) {
                return;
            }
            let active = if turn < waiting { self.waiting.pop_front() } else { Self::take(ports) };
            let Some(mut active) = active else { return };
            if active.request.cancelled() {
                continue;
            }
            if active.program.waiting.is_some() && active.read_closed() {
                self.parked = Some(active.abandon());
                continue;
            }
            if let Some(bytes) = execute(&mut active.program, &mut active.reservation) {
                self.parked = Some(active.reply(bytes));
            } else if active.program.capture.is_none() && active.read_closed() {
                self.parked = Some(active.abandon());
            } else {
                active.reservation.retain(active.program.output.as_ref().map_or(0, crate::netport::Output::capacity));
                self.waiting.push_back(active);
            }
        }
        if self.parked.is_some() || !ports.requests().is_empty() {
            ports.notify_frame();
        }
    }

    fn take(ports: &Ports) -> Option<Active> {
        if ports.requests().is_empty() {
            return None;
        }
        let reservation = ports.budget().reserve_available()?;
        let request = ports.requests().take(1).pop()?;
        let RemoteRequest::Program(text) = request.request() else { return None };
        let mut program = Program::new(text);
        program.output = Some(crate::netport::Output::bounded(reservation.capacity()));
        Some(Active { request, program, reservation })
    }

    #[cfg(test)]
    pub(super) fn active_is_empty(&self) -> bool {
        self.waiting.is_empty() && self.parked.is_none()
    }

    #[cfg(test)]
    pub(super) fn pending(&self) -> bool {
        self.waiting.iter().any(|a| a.program.waiting.is_some())
    }

    #[cfg(test)]
    pub(super) fn pending_client_closed(&self) -> bool {
        self.waiting
            .iter()
            .any(|active| active.program.waiting.is_some() && (active.request.cancelled() || active.read_closed()))
    }
}

impl Active {
    fn read_closed(&self) -> bool {
        self.request.read_closed().load(Ordering::Acquire)
    }

    fn abandon(self) -> Reply {
        let mut reply = Reply::new(self.request.id(), Vec::new(), self.reservation);
        // End the IO session before it can admit programs buffered behind this request.
        reply.set_close(true);
        reply
    }

    fn reply(mut self, bytes: Vec<u8>) -> Reply {
        let mut output = self.program.output.take().unwrap_or_default();
        if !bytes.is_empty() {
            let _ = output.push(bytes);
        }
        if self.program.has_reply {
            let _ = output.push(vec![b'\n']);
        }
        Reply::from_output(self.request.id(), output, self.reservation)
    }
}

#[cfg(test)]
mod tests;

impl super::Scpi {
    pub fn poll(&mut self, ui: &mut Ui, cv: &Canvas) {
        let Some(ports) = self.transport.endpoint.ports(&self.acl) else { return };
        let start = std::time::Instant::now();
        let mut intake = std::mem::take(&mut self.transport.intake);
        intake.poll(
            &ports,
            || start.elapsed(),
            |program, reservation| self.poll_raw_program(ui, cv, program, reservation),
        );
        self.transport.intake = intake;
    }

    fn poll_raw_program(
        &mut self,
        ui: &mut Ui,
        cv: &Canvas,
        program: &mut Program,
        reservation: &mut Reservation,
    ) -> Option<Vec<u8>> {
        if let Some((request, _)) = &mut program.capture {
            let readback = self.readback.as_ref().expect("display requests belong to a device interpreter");
            if !request.ready(readback) {
                return None;
            }
            if !reservation.expand_available(super::message::display_scratch(cv)) {
                self.push_err("-363,\"Output capacity exhausted; remaining commands dropped\"".into());
                program.capture = None;
                program.commands.clear();
                return Some(std::mem::take(&mut program.out));
            }
            if let Some(output) = &mut program.output {
                output.set_limit(reservation.capacity());
            }
        }
        if let Some(fence) = &program.waiting {
            match fence.state(ui) {
                crate::ui::OperationState::Pending => return None,
                crate::ui::OperationState::Cancelled => {}
                crate::ui::OperationState::Complete => {
                    // The reply, separator and newline need space beyond the retained prefix.
                    if !reservation.expand_available(256) {
                        return None;
                    }
                    if let Some(output) = &mut program.output {
                        output.set_limit(reservation.capacity());
                    }
                }
            }
        }
        self.waveform.resume(program.waveform.take());
        let reply = self.poll_program(ui, cv, program);
        if reply.is_none() {
            program.waveform = Some(self.waveform.read());
        }
        self.waveform.resume(None);
        reply
    }

    pub fn open_with_runtime(
        port: impl Into<crate::net6::Port>,
        v6: bool,
        runtime: crate::netio::Runtime,
        wake: std::sync::Arc<crate::system::intake::Wake>,
    ) -> Self {
        let mut scpi = Self::disconnected();
        scpi.transport.endpoint = crate::netio::Endpoint::new(Some(runtime), wake);
        scpi.transport.endpoint.set_open(Some(port.into()), v6, crate::netio::Protocol::Raw);
        scpi
    }

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
