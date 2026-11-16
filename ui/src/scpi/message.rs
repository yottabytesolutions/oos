// SPDX-License-Identifier: GPL-2.0-or-later

//! Whole program messages for callers without a per-client continuation (HTTP, in-process, tests), and the
//! distinction between a command and a query with an empty current result.

use super::*;

impl Scpi {
    /// A response message includes its terminator even when a query's current list is empty; `None`: no query.
    #[cfg(test)]
    pub(crate) fn response(&mut self, ui: &mut Ui, cv: &Canvas, line: &str) -> Option<Vec<u8>> {
        let (mut out, has_reply) = self.exec_message_checked(ui, cv, line).ok()?;
        has_reply.then(|| {
            out.push(b'\n');
            out
        })
    }

    /// Immediate adapter for HTTP and in-process callers. Deferred barriers must use
    /// `poll_program`: returning 1 here while an operation is pending would be false.
    /// Commands before a pending barrier execute once; the remaining message is rejected.
    #[cfg_attr(target_os = "linux", allow(dead_code))] // used by the desktop build and the tests
    pub fn exec_line(&mut self, ui: &mut Ui, cv: &Canvas, line: &str) -> Vec<u8> {
        self.exec_line_checked(ui, cv, line).unwrap_or_default()
    }

    #[cfg_attr(target_os = "linux", allow(dead_code))] // used by the desktop build and the tests
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
        match self.poll_program(ui, cv, &mut program) {
            Some(response) => Ok((response, program.has_reply)),
            None => {
                let error = "Operation pending; use raw TCP or HiSLIP for *OPC?";
                self.push_err(format!("-221,\"{}\"", error));
                self.normalize_errors();
                Err(error)
            }
        }
    }

    pub(super) fn append_program_reply(&mut self, program: &mut Program, response: &[u8]) -> bool {
        let separator = usize::from(program.has_reply);
        if response.len() > RESP_MAX.saturating_sub(program.out.len().saturating_add(separator)) {
            self.push_err("-363,\"Output buffer overrun; remaining commands dropped\"".into());
            program.commands.clear();
            return false;
        }
        program.append(response);
        true
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
