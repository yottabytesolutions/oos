// SPDX-License-Identifier: GPL-2.0-or-later
//! Reversible RUN clock floors. The port owns platform IO; the controller owns saved minima.
mod floor;
mod journal;
mod linux;
mod thermal;
use floor::Floor;
#[cfg(test)]
mod tests;

use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Config {
    cpu: bool,
    gpu: bool,
}
impl Config {
    pub fn parse(conf: &str) -> Self {
        let on = |key| match crate::sysconf::conf_value(None, conf, key).as_deref() {
            None | Some("on") => true,
            Some("off") => false,
            Some(value) => {
                eprintln!("oos-ui: clock_floor {key}={value} unrecognised; disabled");
                false
            }
        };
        Self { cpu: on("perf.cpu_floor"), gpu: on("perf.gpu_floor") }
    }
}

/// Paths are relative to the platform's sysfs root, including in host fixtures.
pub trait Port: Send {
    fn read(&self, path: &str) -> io::Result<String>;
    fn write(&mut self, path: &str, value: u64) -> io::Result<()>;
    fn entries(&self, path: &str) -> io::Result<Vec<String>>;
}

pub struct Controller {
    port: Box<dyn Port>,
    floors: Vec<Floor>,
    journal: journal::Journal,
    thermal: thermal::Guard,
    running: bool,
    recovery_failed: bool,
    requested: bool,
    next_restore: Instant,
    clock: Box<dyn Fn() -> Instant + Send>,
}
impl Controller {
    pub fn new(port: Box<dyn Port>, config: Config, gpu: Option<&str>, saved: std::path::PathBuf) -> Self {
        Self::with_clock(port, config, gpu, saved, Box::new(Instant::now))
    }

    fn with_clock(
        port: Box<dyn Port>,
        config: Config,
        gpu: Option<&str>,
        saved: std::path::PathBuf,
        clock: Box<dyn Fn() -> Instant + Send>,
    ) -> Self {
        let now = clock();
        let mut floors = vec![Floor::new(
            "cpu",
            "devices/system/cpu/cpufreq/policy4",
            "scaling_min_freq",
            "scaling_available_frequencies",
            1_416_000,
        )];
        if let Some(gpu) = gpu {
            floors.push(Floor::new("gpu", gpu, "min_freq", "available_frequencies", 600_000_000));
        }
        let thermal = thermal::Guard::discover(port.as_ref(), now);
        let mut me = Self {
            port,
            floors,
            journal: journal::Journal::new(saved),
            thermal,
            running: false,
            recovery_failed: false,
            requested: false,
            next_restore: now,
            clock,
        };
        // Recover irrespective of current opt-outs, before reading any new previous minimum.
        if let Err(error) = me.recover() {
            eprintln!("oos-ui: clock_floor recovery_error={error}; floors disabled");
            me.recovery_failed = true;
            me.floors.clear();
            return me;
        }
        me.floors.retain(|f| {
            let enabled = if f.name == "cpu" { config.cpu } else { config.gpu };
            if !enabled {
                return false;
            }
            let present = me.port.read(&f.minimum).is_ok() && me.port.read(&f.available).is_ok();
            if !present {
                eprintln!("oos-ui: clock_floor device={} absent; disabled", f.name);
            }
            present
        });
        if config.gpu && gpu.is_none() {
            eprintln!("oos-ui: clock_floor device=gpu absent; disabled");
        }
        me
    }

    fn recover(&mut self) -> io::Result<()> {
        let Some(saved) = self.journal.load()? else { return Ok(()) };
        for (name, value) in saved {
            let floor = self
                .floors
                .iter_mut()
                .find(|f| f.name == name)
                .ok_or_else(|| io::Error::other("saved device is absent"))?;
            floor.saved = Some(value);
            floor.restore(self.port.as_mut())?;
            eprintln!("oos-ui: clock_floor recovery device={name} restored={value}");
        }
        self.journal.clear()
    }

    fn raise(&mut self, index: usize) -> io::Result<()> {
        if self.floors[index].saved.is_some() {
            return Ok(());
        }
        let previous = self.floors[index].prepare(self.port.as_ref())?;
        self.journal.save(&self.floors, index, previous)?;
        self.floors[index].saved = Some(previous);
        self.floors[index].raise(self.port.as_mut())
    }

    pub fn update(&mut self, running: bool) {
        self.requested = running;
        if self.recovery_failed {
            return;
        }
        let now = (self.clock)();
        let allowed = running && !self.floors.is_empty() && self.thermal.allowed(self.port.as_ref(), now);
        if allowed == self.running
            && (allowed || self.floors.iter().all(|f| f.saved.is_none()) || now < self.next_restore)
        {
            return;
        }
        self.running = allowed;
        if !allowed {
            self.next_restore = now + thermal::SAMPLE_PERIOD;
        }
        for index in 0..self.floors.len() {
            let result = if allowed { self.raise(index) } else { self.floors[index].restore(self.port.as_mut()) };
            // Restore reports only the first failure and eventual success for each device.
            if let (true, Err(error)) = (allowed, result) {
                eprintln!("oos-ui: clock_floor device={} running={allowed} error={error}", self.floors[index].name);
            }
        }
        if !allowed && self.floors.iter().all(|f| f.saved.is_none()) {
            if let Err(error) = self.journal.clear() {
                eprintln!("oos-ui: clock_floor journal_error={error}");
            }
        }
    }

    pub fn restore(&mut self) {
        self.running = true;
        self.update(false);
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.restore();
    }
}

/// A process-owned guard shared with the panic hook, including panic=abort builds.
pub struct Handle(Arc<Mutex<Controller>>);
impl Handle {
    #[cfg(test)]
    pub(crate) fn fixture(controller: Controller) -> Self {
        Self(Arc::new(Mutex::new(controller)))
    }
    #[cfg(target_os = "linux")]
    pub fn system(config: Config) -> Self {
        Self::guarded(linux::controller(
            std::path::Path::new("/sys"),
            std::path::Path::new("/tmp/oos-clock-floors"),
            config,
        ))
    }

    #[cfg(any(target_os = "linux", test))]
    fn guarded(controller: Controller) -> Self {
        let handle = Self(Arc::new(Mutex::new(controller)));
        let state = handle.0.clone();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            // A panic during an update must not deadlock its own restoration hook.
            if let Ok(mut controller) = state.try_lock() {
                controller.restore();
            }
            previous(info);
        }));
        handle
    }

    pub fn poll(&self) {
        let mut controller = oos_hal::sync::lock(&self.0);
        let requested = controller.requested;
        controller.update(requested);
    }

    pub fn update(&self, running: bool) {
        oos_hal::sync::lock(&self.0).update(running);
    }
    pub fn restore(&self) {
        oos_hal::sync::lock(&self.0).restore();
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.restore();
    }
}
