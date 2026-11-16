// SPDX-License-Identifier: GPL-2.0-or-later
use super::Port;
use std::io;

pub(super) struct Floor {
    pub(super) name: &'static str,
    pub(super) minimum: String,
    pub(super) available: String,
    pub(super) target: u64,
    pub(super) saved: Option<u64>,
    pub(super) logged: bool,
    restore_failed: bool,
}
impl Floor {
    pub(super) fn new(name: &'static str, dir: &str, minimum: &str, available: &str, target: u64) -> Self {
        Self {
            name,
            minimum: format!("{dir}/{minimum}"),
            available: format!("{dir}/{available}"),
            target,
            saved: None,
            logged: false,
            restore_failed: false,
        }
    }

    pub(super) fn supports(&self, io: &dyn Port, value: u64) -> io::Result<bool> {
        Ok(io.read(&self.available)?.split_whitespace().filter_map(|s| s.parse::<u64>().ok()).any(|v| v == value))
    }

    pub(super) fn prepare(&self, io: &dyn Port) -> io::Result<u64> {
        let previous = io.read(&self.minimum)?.trim().parse::<u64>().map_err(io::Error::other)?;
        let applied = previous.max(self.target);
        if !self.supports(io, applied)? || !self.supports(io, previous)? {
            return Err(io::Error::other("minimum is not in available frequencies"));
        }
        Ok(previous)
    }

    pub(super) fn raise(&mut self, io: &mut dyn Port) -> io::Result<()> {
        let Some(previous) = self.saved else { return Ok(()) };
        let applied = previous.max(self.target);
        // The controller persisted this minimum before calling raise.
        io.write(&self.minimum, applied)?;
        if io.read(&self.minimum)?.trim() != applied.to_string() {
            return Err(io::Error::other("minimum read-back mismatch"));
        }
        if !self.logged {
            eprintln!("oos-ui: clock_floor device={} applied={} previous={}", self.name, applied, previous);
            self.logged = true;
        }
        Ok(())
    }

    pub(super) fn restore(&mut self, io: &mut dyn Port) -> io::Result<()> {
        let Some(previous) = self.saved else {
            return Ok(());
        };
        let result = self.restore_value(io, previous);
        match &result {
            Err(error) if !self.restore_failed => {
                eprintln!("oos-ui: clock_floor device={} running=false error={error}", self.name);
            }
            Ok(()) if self.restore_failed => {
                eprintln!("oos-ui: clock_floor device={} restored={previous}", self.name);
            }
            _ => {}
        }
        self.restore_failed = result.is_err();
        result
    }

    fn restore_value(&mut self, io: &mut dyn Port, previous: u64) -> io::Result<()> {
        if !self.supports(io, previous)? {
            return Err(io::Error::other("saved minimum is no longer available"));
        }
        io.write(&self.minimum, previous)?;
        if io.read(&self.minimum)?.trim() != previous.to_string() {
            return Err(io::Error::other("restore read-back mismatch"));
        }
        self.saved = None;
        Ok(())
    }
}
