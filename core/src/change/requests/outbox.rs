// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Error, Request};
use crate::model::{Channel, Model, RunState, TrigMode};

/// Narrow transitional proposal values. Grouped command APIs arrive with the later settings migration.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    Run(RunState),
    Timebase(usize, f32),
    TriggerMode(TrigMode),
    Channel(usize, Channel),
    AutoFast(bool),
    Request(Request),
    Patch(crate::change::Patch),
}
impl Command {
    pub fn apply(&self, model: &mut Model) {
        match self {
            Self::Run(run) => model.run = *run,
            Self::Timebase(scale, delay) => {
                model.tscale = *scale;
                model.delay = *delay;
            }
            Self::TriggerMode(mode) => model.trig_mode = *mode,
            Self::Channel(i, ch) => {
                if let Some(c) = model.ch.get_mut(*i) {
                    c.clone_from(ch);
                }
            }
            Self::AutoFast(on) => model.auto_fast = *on,
            Self::Request(_) => {}
            Self::Patch(patch) => patch.apply(model),
        }
    }
}

/// 64 entries and 64 KiB including reserved storage. Nested command payload capacities are included in the byte cap.
#[derive(Clone, Debug, PartialEq)]
pub struct Outbox {
    entries: Vec<Command>,
    used: usize,
}
impl Default for Outbox {
    fn default() -> Self {
        Self { entries: Vec::with_capacity(64), used: 0 }
    }
}
impl Outbox {
    pub fn push(&mut self, command: Command) -> Result<(), Error> {
        if self.used == 64 {
            return Err(Error::Full);
        }
        if self.bytes() + payload(&command) >= 64 * 1024 {
            return Err(Error::PayloadTooLarge);
        }
        self.store(command)?;
        self.used += 1;
        Ok(())
    }
    /// Capture effective legacy writes as typed proposals. Slots and nested capacities are retained after clear.
    pub fn capture(&mut self, before: &Model, after: &Model) -> Result<(), Error> {
        let mut result = Ok(());
        crate::change::fields(before, after, |f| {
            if result.is_ok()
                && f.changed()
                && f.index() < 194
                && !matches!(f.name(), "status" | "hw_stamp" | "hw_avg_clear")
            {
                result = self.capture_field(f.index(), after);
            }
        });
        result
    }
    fn capture_field(&mut self, index: usize, model: &Model) -> Result<(), Error> {
        if self.used == 64 {
            return Err(Error::Full);
        }
        if self.bytes() + crate::change::snapshot_bytes(model) >= 64 * 1024 {
            return Err(Error::PayloadTooLarge);
        }
        if let Some(i) =
            self.entries[self.used..].iter().position(|c| matches!(c, Command::Patch(p) if p.index() == index))
        {
            self.entries.swap(self.used, self.used + i);
            if let Command::Patch(patch) = &mut self.entries[self.used] {
                patch.reuse(model);
            }
            self.used += 1;
            return Ok(());
        }
        if let Some(patch) = crate::change::Patch::capture(index, model) {
            self.push(Command::Patch(patch))?;
        }
        Ok(())
    }
    fn store(&mut self, command: Command) -> Result<(), Error> {
        // Preserve warmed heap-owning patches even when different fields change on successive steps.
        if let Some(i) = self.entries[self.used..].iter().position(|c| payload(c) == 0) {
            self.entries.swap(self.used, self.used + i);
            self.entries[self.used] = command;
        } else if self.entries.len() < 64 {
            self.entries.push(command);
            let last = self.entries.len() - 1;
            self.entries.swap(self.used, last);
        } else {
            return Err(Error::Full);
        }
        Ok(())
    }
    pub fn entries(&self) -> &[Command] {
        &self.entries[..self.used]
    }
    pub fn clear(&mut self) {
        self.used = 0;
    }
    pub fn apply(&self, model: &mut Model) {
        for command in self.entries() {
            command.apply(model);
        }
    }
    pub fn requests(&self, out: &mut [Request; 64]) -> usize {
        let mut n = 0;
        for command in self.entries() {
            if let Command::Request(r) = command {
                out[n] = *r;
                n += 1;
            }
        }
        n
    }
    pub fn bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.entries.capacity() * std::mem::size_of::<Command>()
            + self.entries.iter().map(payload).sum::<usize>()
    }
}

fn payload(command: &Command) -> usize {
    match command {
        Command::Patch(patch) => patch.bytes(),
        _ => 0,
    }
}
