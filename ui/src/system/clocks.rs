// SPDX-License-Identifier: GPL-2.0-or-later
//! Reversible UI clock floors. The port owns platform IO; the controller owns saved minima.
mod floor;
mod journal;
mod linux;
mod status;
mod thermal;
use floor::Floor;
use status::{Reason, Snapshot, Status};
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

#[derive(Clone, Copy, PartialEq)]
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

enum Recovery {
    Ready,
    Pending(Instant),
    Blocked,
}

pub struct Controller {
    port: Box<dyn Port>,
    floors: [Floor; 2],
    journal: journal::Journal,
    recovery: Recovery,
    request: Request,
    critical: thermal::Guard,
    next_receipt_sync: Option<Instant>,
    snapshot: Snapshot,
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
            recovery: Recovery::Ready,
            request: Request::Off,
            critical,
            next_receipt_sync: None,
            snapshot: Snapshot::new(now),
            clock,
        };
        // Recover irrespective of current opt-outs, before reading any new previous minimum.
        me.start_recovery(now);
        me.sample(true, now);
        me
    }

    fn start_recovery(&mut self, now: Instant) {
        let saved = match self.journal.load() {
            Ok(None) => return,
            Ok(Some(saved)) => saved,
            Err(error) => {
                eprintln!("oos-ui: clock_floor recovery_error={error}; floors disabled");
                self.recovery = Recovery::Blocked;
                return;
            }
        };
        for (name, value) in saved {
            // The journal validates the device names before handing over ownership.
            if let Some(floor) = self.floors.iter_mut().find(|f| f.name == name) {
                floor.claim(value);
            }
        }
        if let Err(error) = self.restore_receipt() {
            eprintln!("oos-ui: clock_floor recovery_error={error}; floors disabled");
            self.recovery = Recovery::Pending(now + thermal::SAMPLE_PERIOD);
        }
    }

    fn restore_receipt(&mut self) -> io::Result<()> {
        let mut failure = None;
        for floor in &mut self.floors {
            if let Err(error) = floor.recover(self.port.as_mut()) {
                failure.get_or_insert(error);
            }
        }
        if let Some(error) = failure {
            return Err(error);
        }
        // Keep the startup receipt until every saved minimum has been restored or its device is absent.
        self.journal.clear()
    }

    fn apply(&mut self, index: usize, target: u64) -> io::Result<()> {
        let floor = &self.floors[index];
        let previous = floor.prepare(self.port.as_ref(), target)?;
        if floor.saved().is_none() {
            self.journal.save(&self.floors, index, previous)?;
            self.floors[index].claim(previous);
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
        let changed = request != self.request;
        self.request = request;
        let now = (self.clock)();
        let updated = match self.recovery {
            Recovery::Ready => self.update_floors(request, now, changed, force),
            Recovery::Pending(next) if force || now >= next => {
                self.recovery = if self.restore_receipt().is_ok() {
                    Recovery::Blocked
                } else {
                    Recovery::Pending(now + thermal::SAMPLE_PERIOD)
                };
                true
            }
            _ => false,
        };
        self.sample(changed || updated, now);
    }

    fn update_floors(&mut self, request: Request, now: Instant, changed: bool, force: bool) -> bool {
        let critical = match request {
            Request::Off => None,
            _ => self.critical.inhibit(self.port.as_ref(), "all", now),
        };
        let mut updated = false;
        for (index, target) in request.minima().into_iter().enumerate() {
            updated |= self.update_floor(index, target, critical, now, changed, force);
        }
        if self.next_receipt_sync.is_some_and(|next| force || now >= next) {
            self.next_receipt_sync = self.journal.sync(&self.floors).err().map(|error| {
                eprintln!("oos-ui: clock_floor journal_error={error}");
                now + thermal::SAMPLE_PERIOD
            });
        }
        updated
    }

    fn update_floor(
        &mut self,
        index: usize,
        target: Option<u64>,
        critical: Option<Reason>,
        now: Instant,
        changed: bool,
        force: bool,
    ) -> bool {
        let floor = &mut self.floors[index];
        let state = floor.state();
        let inhibit = floor.inhibit(self.port.as_ref(), target.is_some(), critical, now);
        let retry_restore = floor.restore_failed() && !changed && state != Reason::ApplyFailed;
        let mut updated = false;
        if inhibit.is_some() || retry_restore {
            let released = floor.release(self.port.as_mut(), inhibit, now, force);
            if released == Some(true) {
                self.next_receipt_sync = Some(now);
            }
            updated = released.is_some() || state != floor.state();
            if inhibit.is_some() || floor.restore_failed() {
                return updated;
            }
        }
        if let Some(target) = target.filter(|&target| changed || floor.should_apply(target, now)) {
            let lowering = floor.lowering(target);
            let result = self.apply(index, target);
            let failed_lowering = lowering && result.is_err();
            let floor = &mut self.floors[index];
            floor.finish_apply(target, now, result);
            // If STOP cannot verify the display floor, release the RUN floor to its saved minimum.
            if failed_lowering && floor.release_failed_lowering(self.port.as_mut()) {
                self.next_receipt_sync = Some(now);
            }
            updated = true;
        }
        updated
    }

    fn sample(&mut self, changed: bool, now: Instant) {
        let minima = self.request.minima();
        let blocked = !matches!(self.recovery, Recovery::Ready);
        self.snapshot.refresh(changed, now, || {
            std::array::from_fn(|index| self.floors[index].status(self.port.as_ref(), minima[index], blocked))
        });
    }
}
impl Drop for Controller {
    fn drop(&mut self) {
        self.restore();
    }
}

/// A process-owned guard shared with the panic hook, including panic=abort builds.
pub struct Handle {
    controller: Arc<Mutex<Controller>>,
    readings: status::Readings,
}
impl Handle {
    #[cfg(any(target_os = "linux", test))]
    fn new(controller: Controller) -> Self {
        let readings = controller.snapshot.readings().clone();
        Self { controller: Arc::new(Mutex::new(controller)), readings }
    }

    #[cfg(test)]
    pub(crate) fn fixture(controller: Controller) -> Self {
        Self::new(controller)
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
        let handle = Self::new(controller);
        let state = handle.controller.clone();
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
        *oos_hal::sync::lock(&self.readings)
    }

    pub fn poll(&self) {
        let mut controller = oos_hal::sync::lock(&self.controller);
        let request = controller.request;
        controller.tick(request, false);
    }

    pub fn update(&self, running: bool) {
        oos_hal::sync::lock(&self.controller).update(running);
    }
    pub fn restore(&self) {
        oos_hal::sync::lock(&self.controller).restore();
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
