// SPDX-License-Identifier: GPL-2.0-or-later

//! Whole program messages for callers without a per-client continuation (HTTP, in-process, tests), and the
//! distinction between a command and a query with an empty current result.

use super::*;

impl Scpi {
    /// An in-process interpreter with no transport or listener.
    pub fn disconnected() -> Self {
        Self {
            transport: Default::default(),
            waveform: Default::default(),
            err: Vec::new(),
            event_status: 0,
            error_serial: 0,
            last_error: None,
            opc_events: Vec::new(),
            readback: None,
            acl: crate::netacl::Shared::default(),
        }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn use_readback(&mut self, readback: crate::system::readback::Readback) {
        self.readback = Some(readback);
    }

    pub(crate) fn readback(&self) -> Option<&crate::system::readback::Readback> {
        self.readback.as_ref()
    }

    /// A response message includes its terminator even when a query's current list is empty; `None`: no query.
    #[cfg(test)]
    pub(crate) fn response(&mut self, ui: &mut Ui, cv: &Canvas, line: &str) -> Option<Vec<u8>> {
        let (mut out, has_reply) = self.exec_message_checked(ui, cv, line).ok()?;
        has_reply.then(|| {
            out.push(b'\n');
            out
        })
    }

    #[cfg(test)]
    /// Immediate adapter for HTTP and in-process callers. Deferred barriers must use
    /// `poll_program`: returning 1 here while an operation is pending would be false.
    /// Commands before a pending barrier execute once; the remaining message is rejected.
    pub fn exec_line(&mut self, ui: &mut Ui, cv: &Canvas, line: &str) -> Vec<u8> {
        self.exec_line_checked(ui, cv, line).unwrap_or_default()
    }

    #[cfg(test)]
    pub fn exec_line_checked(&mut self, ui: &mut Ui, cv: &Canvas, line: &str) -> Result<Vec<u8>, &'static str> {
        self.exec_message_checked(ui, cv, line).map(|(response, _)| response)
    }

    pub fn exec_message_checked(
        &mut self,
        ui: &mut Ui,
        cv: &Canvas,
        line: &str,
    ) -> Result<(Vec<u8>, bool), &'static str> {
        let mut program = Program::new(line);
        self.poll_message(ui, cv, &mut program).unwrap_or(Err("Display capture pending"))
    }

    /// HTTP can await display pixels in its own request. *OPC? retains its
    /// existing immediate error contract on this transport.
    pub(crate) fn poll_message(
        &mut self,
        ui: &mut Ui,
        cv: &Canvas,
        program: &mut Program,
    ) -> Option<Result<(Vec<u8>, bool), &'static str>> {
        match self.poll_program(ui, cv, program) {
            Some(response) => Some(Ok((response, program.has_reply))),
            None if program.capture.is_some() => None,
            None => {
                let error = "Operation pending; use raw TCP or HiSLIP for *OPC?";
                self.push_err(format!("-221,\"{}\"", error));
                self.normalize_errors();
                Some(Err(error))
            }
        }
    }

    /// Advance a bounded per-client message without waiting on the UI thread.
    /// The continuation owns its earlier replies and remaining commands: no write is replayed.
    pub fn poll_program(&mut self, ui: &mut Ui, cv: &Canvas, program: &mut Program) -> Option<Vec<u8>> {
        let response = crate::ui::change::intake(ui, |ui| self.advance_program(ui, cv, program));
        self.transport.reply_limit = RESP_MAX;
        self.normalize_errors();
        response
    }

    fn advance_program(&mut self, ui: &mut Ui, cv: &Canvas, program: &mut Program) -> Option<Vec<u8>> {
        self.poll_events(ui);
        if program.invalid {
            self.push_err("-363,\"Program message too large or too many commands\"".into());
            return Some(Vec::new());
        }
        if let Some((mut request, format)) = program.capture.take() {
            let readback = self.readback.as_ref().expect("display requests belong to a device interpreter");
            let Some(response) = request.pixels(readback).map(|pixels| format.encode(pixels)) else {
                program.capture = Some((request, format));
                return None;
            };
            if !self.append_program_reply(program, response) {
                return Some(std::mem::take(&mut program.out));
            }
        }
        self.advance_commands(ui, cv, program)
    }

    fn advance_commands(&mut self, ui: &mut Ui, cv: &Canvas, program: &mut Program) -> Option<Vec<u8>> {
        if let Some(fence) = &program.waiting {
            match fence.state(ui) {
                crate::ui::OperationState::Pending => return None,
                crate::ui::OperationState::Complete => {
                    program.waiting = None;
                    if !self.append_program_reply(program, b"1".to_vec()) {
                        return Some(std::mem::take(&mut program.out));
                    }
                }
                crate::ui::OperationState::Cancelled => {
                    self.push_err("-410,\"OPC query interrupted by cancelled operation\"".into());
                    program.commands.clear();
                    program.out.clear();
                    if program.output.is_some() {
                        program.output = Some(Default::default());
                    }
                    program.has_reply = false;
                    program.waiting = None;
                    return Some(Vec::new());
                }
            }
        }
        while let Some((command, new_line)) = program.commands.pop_front() {
            if new_line && program.has_reply {
                program.separator = b'\n';
            }
            if program.out.len() >= RESP_MAX {
                self.push_err("-363,\"Input buffer overrun\" rest of the line dropped".into());
                program.commands.clear();
                break;
            }
            if !self.prepare_program_output(program, &command, cv) {
                break;
            }
            program.before(ui, &command);
            if command.eq_ignore_ascii_case("*OPC?") {
                let fence = ui.scpi_operations();
                if fence.state(ui) == crate::ui::OperationState::Pending {
                    program.waiting = Some(fence);
                    return None;
                }
                if !self.append_program_reply(program, b"1".to_vec()) {
                    break;
                }
            } else if let Some((request, format)) = self.display_capture(&command) {
                program.capture = Some((request, format));
                return None;
            } else if let Some(response) = self.exec(ui, cv, &command) {
                if !self.append_program_reply(program, response) {
                    break;
                }
            }
        }
        self.transport.reply_limit = RESP_MAX;
        program.settle(ui);
        Some(std::mem::take(&mut program.out))
    }

    fn display_capture(&self, command: &str) -> Option<(crate::system::readback::Request, display::Format)> {
        let readback = self.readback.as_ref()?;
        let format = display::capture_format(command)?;
        Some((readback.request(), format))
    }

    pub(super) fn logic_block(&mut self, words: &[u16]) -> Option<Vec<u8>> {
        let capacity = words.len().saturating_mul(2).saturating_add(11);
        if capacity > self.transport.reply_limit {
            self.push_err("-363,\"Output capacity exhausted\"".into());
            return None;
        }
        let mut bytes = Vec::with_capacity(capacity);
        bytes.extend_from_slice(format!("#9{:09}", words.len() * 2).as_bytes());
        for word in words {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        Some(bytes)
    }

    pub(super) fn prepare_program_output(&mut self, program: &mut Program, command: &str, cv: &Canvas) -> bool {
        let remaining = program.output.as_ref().map_or(RESP_MAX, |output| output.remaining());
        let header = command.split_whitespace().next().unwrap_or("");
        let nodes: Vec<_> = header.trim_start_matches(':').split(':').collect();
        let display = display::data_header(header);
        let waveform = nodes.len() == 2 && kw(nodes[0], "WAVeform") && kw(nodes[1].trim_end_matches('?'), "DATA");
        // PNG's raw, deflate and container buffers overlap; BMP may reallocate its block header.
        let scratch = if display {
            display_scratch(cv)
        } else if waveform {
            256
        } else {
            8 << 20
        };
        if program.output.is_some() && header.ends_with('?') && remaining < scratch {
            self.push_err("-363,\"Output capacity exhausted; remaining commands dropped\"".into());
            program.commands.clear();
            return false;
        }
        self.transport.reply_limit = if program.output.is_some() { remaining.saturating_sub(128) } else { remaining };
        true
    }

    pub(super) fn append_program_reply(&mut self, program: &mut Program, response: Vec<u8>) -> bool {
        let separator = usize::from(program.has_reply);
        let remaining = program.output.as_ref().map_or_else(
            || RESP_MAX.saturating_sub(program.out.len().saturating_add(separator)),
            |output| output.remaining().saturating_sub(separator + 64),
        );
        let bytes = if program.output.is_some() { response.capacity() } else { response.len() };
        if bytes > remaining {
            self.push_err("-363,\"Output buffer overrun; remaining commands dropped\"".into());
            program.commands.clear();
            return false;
        }
        program.append(response)
    }

    pub(super) fn normalize_errors(&mut self) {
        // a client that never asks for its errors, or sends megabytes of nonsense, must not fill the memory
        for e in &mut self.err {
            if e.len() > ERR_LEN {
                *e = e.chars().take(ERR_LEN).collect();
            }
        }
        if self.err.len() > ERR_MAX {
            self.err.truncate(ERR_MAX - 1);
            self.push_err("-350,\"Queue overflow\"".into());
        }
    }
}

pub(super) fn display_scratch(cv: &Canvas) -> usize {
    cv.px.len().saturating_mul(24).saturating_add(1 << 20)
}

/// SCPI is sequential (IEEE 488.2): a query sees the settings and resets of every command before it, and a
/// program message's commands are accepted by its end. A message of queries alone accepts nothing.
impl Program {
    pub(super) fn before(&mut self, ui: &mut Ui, command: &str) {
        if command.split_whitespace().next().unwrap_or("").ends_with('?') {
            self.settle(ui);
        } else {
            self.unsettled = true;
        }
    }

    pub(super) fn settle(&mut self, ui: &mut Ui) {
        if std::mem::take(&mut self.unsettled) {
            change::settle(ui);
        }
    }
}

/// A measurement query (`:MEASure...?`) of a large record waits, bounded, for its background measurement
/// (`Ui::measurement_settle`); it never answers with the value of other settings or another record.
pub(super) fn settle_measurements(ui: &mut Ui, cmd: &str) {
    let head = cmd.split_whitespace().next().unwrap_or("");
    let first = head.trim_start_matches(':').split(':').next().unwrap_or("").trim_end_matches('?');
    if head.ends_with('?') && kw(first, "MEASure") {
        ui.measurement_settle();
    }
}
