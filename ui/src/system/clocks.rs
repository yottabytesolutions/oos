// SPDX-License-Identifier: GPL-2.0-or-later
//! Reversible UI clock floors. The port owns platform IO; the controller owns saved minima.
mod floor;
mod journal;
mod linux;
mod status;
mod thermal;
use floor::Floor;
use status::Status;
#[cfg(test)]
mod tests;

use std::io;
use std::path::PathBuf;
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
    /// A port without link metadata cannot establish a cooling binding.
    fn read_link(&self, _path: &str) -> io::Result<PathBuf> {
        Err(io::ErrorKind::Unsupported.into())
    }
}

#[derive(Clone, Copy)]
enum Request {
    Off,
    Draw,
    Run,
}
impl Request {
    // Policy4 uses kHz and devfreq uses Hz.
    fn minima(self) -> [Option<u64>; 2] {
        match self {
            Self::Off => [None, None],
            Self::Draw => [None, Some(400_000_000)],
            Self::Run => [Some(1_416_000), Some(600_000_000)],
        }
    }
}

pub struct Controller {
    port: Box<dyn Port>,
    floors: [Floor; 2],
    journal: journal::Journal,
    recovery_failed: bool,
    request: Request,
    critical: thermal::Guard,
    next_receipt_sync: Option<Instant>,
    clock: Box<dyn Fn() -> Instant + Send>,
}
impl Controller {
    pub fn new(port: Box<dyn Port>, config: Config, gpu: Option<&str>, saved: PathBuf) -> Self {
        Self::with_clock(port, config, gpu, saved, Box::new(Instant::now))
    }

    pub(crate) fn with_clock(
        port: Box<dyn Port>,
        config: Config,
        gpu: Option<&str>,
        saved: PathBuf,
        clock: Box<dyn Fn() -> Instant + Send>,
    ) -> Self {
        let now = clock();
        let floors = [Floor::cpu(port.as_ref(), config.cpu, now), Floor::gpu(port.as_ref(), config.gpu, gpu, now)];
        let critical = thermal::Guard::critical(port.as_ref(), now);
        let mut me = Self {
            port,
            floors,
            journal: journal::Journal::new(saved),
            recovery_failed: false,
            request: Request::Off,
            critical,
            next_receipt_sync: None,
            clock,
        };
        // Recover irrespective of current opt-outs, before reading any new previous minimum.
        if let Err(error) = me.recover() {
            eprintln!("oos-ui: clock_floor recovery_error={error}; floors disabled");
            me.recovery_failed = true;
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

    fn apply(&mut self, index: usize, target: u64) -> io::Result<()> {
        let floor = &self.floors[index];
        let previous = floor.prepare(self.port.as_ref(), target)?;
        if floor.saved.is_none() {
            self.journal.save(&self.floors, index, previous)?;
            self.next_receipt_sync = None;
        }
        self.floors[index].apply(self.port.as_mut(), previous, target)
    }

    /// Update the request for a drawing UI. Hardware RUN also requests the CPU floor.
    pub fn update(&mut self, running: bool) {
        self.tick(if running { Request::Run } else { Request::Draw }, false);
    }

    pub fn restore(&mut self) {
        self.tick(Request::Off, true);
    }

    fn tick(&mut self, request: Request, force: bool) {
        self.request = request;
        if self.recovery_failed {
            return;
        }
        let now = (self.clock)();
        let critical = match request {
            Request::Off => None,
            _ => self.critical.inhibit(self.port.as_ref(), "all", now),
        };
        for (index, target) in request.minima().into_iter().enumerate() {
            self.update_floor(index, target, critical, now, force);
        }
        if self.next_receipt_sync.is_some_and(|next| force || now >= next) {
            self.next_receipt_sync = self.journal.sync(&self.floors).err().map(|error| {
                eprintln!("oos-ui: clock_floor journal_error={error}");
                now + thermal::SAMPLE_PERIOD
            });
        }
    }

    fn update_floor(
        &mut self,
        index: usize,
        target: Option<u64>,
        critical: Option<status::Reason>,
        now: Instant,
        force: bool,
    ) {
        let floor = &mut self.floors[index];
        let inhibit = floor.inhibit(self.port.as_ref(), target.is_some(), critical, now);
        if inhibit.is_some() || floor.restore_failed() {
            if floor.release(self.port.as_mut(), inhibit, now, force) {
                self.next_receipt_sync = Some(now);
            }
            if inhibit.is_some() || floor.restore_failed() {
                return;
            }
        }
        if let Some(target) = target.filter(|&target| floor.should_apply(target, now)) {
            let result = self.apply(index, target);
            self.floors[index].finish_apply(target, now, result);
        }
    }

    fn status(&self) -> [Status; 2] {
        let minima = self.request.minima();
        std::array::from_fn(|index| self.floors[index].status(self.port.as_ref(), minima[index], self.recovery_failed))
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

    fn status(&self) -> [Status; 2] {
        oos_hal::sync::lock(&self.0).status()
    }

    pub fn poll(&self) {
        let mut controller = oos_hal::sync::lock(&self.0);
        let request = controller.request;
        controller.tick(request, false);
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

pub(crate) fn with_status(mut text: String, handle: Option<&Handle>) -> String {
    let floors = handle.map_or_else(Status::absent, Handle::status);
    for floor in floors {
        floor.write_scpi(&mut text);
    }
    text
}
