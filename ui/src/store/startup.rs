// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! One owner of automatic setup promotion and the running/stable marker lifecycle (ADR 0017).
use super::durable::{self, Disk, Files};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(test)]
mod tests;

pub(crate) mod crash;
mod quarantine;
pub(crate) mod worker;

const STABLE: Duration = Duration::from_secs(60);

struct Paths {
    data: PathBuf,
    temporary: PathBuf,
}

pub(crate) struct Startup<F = Disk> {
    fs: F,
    paths: Paths,
    run: String,
    candidate: Option<(String, Duration)>,
    persisted: Option<String>,
    stable: bool,
    safe: bool,
    disabled: bool,
    retry: Retry,
}

#[derive(Default)]
struct Retry {
    at: Duration,
    delay: Duration,
}

impl<F: Files> Startup<F> {
    pub(crate) fn start(fs: F, data: &Path, temporary: &Path, run: String, safe: bool) -> io::Result<Self> {
        let mut s = Self {
            fs,
            paths: Paths { data: data.into(), temporary: temporary.into() },
            run,
            candidate: None,
            persisted: None,
            stable: false,
            safe,
            disabled: false,
            retry: Retry::default(),
        };
        if let Err(e) = s.begin() {
            eprintln!("oos-ui: persistence off: {}", e);
            s.disabled = true;
        }
        Ok(s)
    }

    fn begin(&mut self) -> io::Result<()> {
        if self.run.is_empty() {
            return Err(io::Error::other("no run identity"));
        }
        // Precedes setup parsing and source creation. Keep the prior stable marker until it commits.
        durable::replace(&mut self.fs, &self.paths.data.join("oos-ui.running"), &self.run)?;
        durable::remove(&mut self.fs, &self.paths.data.join("oos-ui.stable"))?;
        if self.safe {
            self.quarantine()?;
        }
        Ok(())
    }

    pub(crate) fn enabled(&self) -> bool {
        !self.disabled
    }

    pub(crate) fn restore(&self) -> io::Result<Option<String>> {
        if self.safe || self.disabled {
            return Ok(None);
        }
        match self.fs.read(&self.paths.temporary.join("oos-ui.setup")) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn restore_model(&self) -> io::Result<Option<crate::model::Model>> {
        // what this build cannot take is left out, the rest stands (upstream 0.1.13, `store::parse_setup_lenient`)
        Ok(self.restore()?.map(|text| {
            let (model, bad) = super::parse_setup_lenient(&text);
            for e in &bad {
                eprintln!("oos-ui: restore: left out: {}", e);
            }
            model
        }))
    }

    pub(crate) fn restored_model(&self) -> Option<crate::model::Model> {
        match self.restore_model() {
            Ok(model) => model,
            Err(e) => {
                eprintln!("oos-ui: restore: {}", e);
                None
            }
        }
    }

    /// Observe every step, not just each disk-write interval: even a transient setting restarts the timer.
    pub(crate) fn observe(&mut self, text: String, now: Duration) -> io::Result<()> {
        if self.candidate.as_ref().is_none_or(|(old, _)| *old != text) {
            self.candidate = Some((text, now));
        }
        if self.disabled || now < self.retry.at {
            return Ok(());
        }
        let Some((text, since)) = self.candidate.as_ref() else { return Ok(()) };
        if now.saturating_sub(*since) < STABLE {
            return Ok(());
        }
        let text = text.clone();
        match self.promote(&text) {
            Ok(()) => {
                self.retry = Retry::default();
                Ok(())
            }
            Err(e) => {
                let first = self.retry.delay.is_zero();
                self.retry.delay = (self.retry.delay * 2).max(Duration::from_secs(1)).min(Duration::from_secs(300));
                self.retry.at = now + self.retry.delay;
                if first {
                    Err(e)
                } else {
                    Ok(())
                }
            }
        }
    }

    fn promote(&mut self, text: &str) -> io::Result<()> {
        self.persist(text)?;
        if !self.stable {
            durable::replace(&mut self.fs, &self.paths.data.join("oos-ui.stable"), &self.run)?;
            durable::remove(&mut self.fs, &self.paths.data.join("oos-ui.running"))?;
            self.stable = true;
        }
        Ok(())
    }

    fn persist(&mut self, text: &str) -> io::Result<()> {
        if self.persisted.as_deref() == Some(text) {
            return Ok(());
        }
        durable::replace(&mut self.fs, &self.paths.temporary.join("oos-ui.setup"), text)?;
        durable::replace(&mut self.fs, &self.paths.data.join("oos-ui.setup"), text)?;
        self.persisted = Some(text.into());
        Ok(())
    }

    /// A clean power-off blesses the current setup; an ordinary clean stop only promotes stable settings.
    pub(crate) fn clean(&mut self, text: String, now: Duration, power_off: bool) -> io::Result<()> {
        if self.disabled {
            return Ok(());
        }
        if power_off {
            self.persist(&text)?;
        } else {
            self.observe(text, now)?;
        }
        self.clear_markers()
    }

    fn clear_markers(&mut self) -> io::Result<()> {
        if self.disabled {
            return Ok(());
        }
        durable::remove(&mut self.fs, &self.paths.data.join("oos-ui.running"))?;
        durable::remove(&mut self.fs, &self.paths.data.join("oos-ui.stable"))
    }
}

impl Startup {
    /// Scope startup only. Scripts and host servers neither read nor write the unit's recovery state.
    pub(crate) fn device(args: &[String]) -> Option<Self> {
        if !cfg!(target_os = "linux") || args.iter().any(|s| matches!(s.as_str(), "--script" | "--serve")) {
            return None;
        }
        let run = std::env::var("OOS_UI_RUN_ID")
            .or_else(|_| {
                std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
                    .map(|boot| format!("{}:{}", boot.trim(), std::process::id()))
            })
            .unwrap_or_default();
        let safe = args.iter().any(|s| s == "--safe");
        Self::start(Disk, Path::new("/oos"), Path::new("/tmp"), run.trim().into(), safe).ok()
    }
}
