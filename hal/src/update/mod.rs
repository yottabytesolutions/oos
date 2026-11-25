// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Signed OOS updates with rollback (docs/update.md).
//!
//! - `pkg`: the signed package `.oosupd` (Ed25519 over a text header that names the image's SHA-256).
//! - `card`: the card layout, the image regions (`recovery` and the rollback slot in `backup`) and the boot magic.
//! - `first`: the first install from the running stock system, without opening the scope.
//! - `kexec`: stage and start a candidate while leaving recovery intact.
//! - here: the state file (/oos/update/state) and the operations `install`, `rollback`, `boot_started`, `judge`,
//!   `confirm`.
//!
//! Safety net, from the bottom up:
//! 1. The stock loader checks every image's id; a damaged `recovery` falls back to the stock copy in `backup`.
//! 2. `oos-oneshot` (first line of rcS) puts `KRNL` back at every OOS start, so a start that hangs after it is
//!    followed by stock Android at the next power-on. Before it, the loader alone decides: a kernel or initramfs that
//!    hangs before rcS would start again at every power-on. That is why a new image is first started by kexec
//!    (`kexec_stage`), with `recovery` untouched, and installs itself only from that start.
//! 3. A new image runs on trial: the watcher (`oos-update watch`) confirms it once oos-ui ran 20 s and the
//!    start-up finished; a crash loop or a start-up that does not finish in 180 s writes the previous image back at
//!    once. A trial image that comes up a second time without having been confirmed rolls back at that start.
//! 4. Persistent mode (`mode persist`) arms OOS again after every confirmed start, so OOS stays the system without
//!    losing step 2.

pub mod card;
pub mod ed25519;
pub mod fdt;
pub mod first;
pub mod hash;
pub mod inflate;
pub mod kexec;
pub mod models;
pub mod pkg;
pub mod sd;
pub mod text;
pub use card::Card;
use card::{check_image, Region};
pub use kexec::receive::kexec_clear;
pub use reconcile::{leave, reconcile_recovery};
pub use restart_mark::{read_restart_mark, write_restart_mark};
pub use state::{confirm, confirm_running, confirm_with_sd_mode};
use std::path::Path;

/// Default place of the state file (OOS data partition).
pub const STATE_PATH: &str = "/oos/update/state";
/// Start-up watcher completion marker, shared with the panel power dialog.
pub const JUDGED_PATH: &str = "/tmp/oos-update.judged";
/// The keys every OOS build trusts (oos/keys/update.pub).
pub const TRUSTED_KEYS: &str = include_str!("../../../keys/update.pub");

pub fn trusted_keys() -> Vec<[u8; 32]> {
    pkg::parse_keys(TRUSTED_KEYS)
}

/// One image the state knows.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub build: String,
    pub sha256: [u8; 32],
    pub len: u64,
}

impl Entry {
    fn build_key(&self) -> &str {
        self.build.split_whitespace().next().unwrap_or("")
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
struct RecoveryTracking {
    loader_id: Option<[u8; 20]>,
    untracked: Option<String>,
    writing: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    /// The tracked image in `recovery`.
    pub current: Option<Entry>,
    /// The image in the rollback slot.
    pub previous: Option<Entry>,
    /// Some(n): `current` is on trial, started n times without being confirmed.
    pub trial: Option<u32>,
    /// Arm OOS again after every confirmed start.
    pub persist: bool,
    sd: sd::Tracking,
    /// The build whose kexec start is under way.
    pub kexec: Option<String>,
    kexec_payload: Option<usize>,
    staged: Option<Entry>,
    // The slot may contain an incomplete write or a discarded candidate.
    slot_incomplete: bool,
    recovery: RecoveryTracking,
    /// Last thing that happened, one line.
    pub event: String,
}

mod reconcile;
mod restart_mark;
mod state;

/// Short form of a build id for messages.
pub fn short(build: &str) -> &str {
    build.split_whitespace().next().unwrap_or(build)
}

#[derive(Clone, Copy, Debug)]
pub struct InstallOpt<'a> {
    /// Detected hardware model; card-file tests supply their simulated model.
    pub running_model: &'a str,
    pub execute: bool,
    /// Allow an older build than the current one.
    pub downgrade: bool,
    /// Install even when `recovery` already holds this image.
    pub force: bool,
    /// Allow a package built for another model, with an override warning in the report.
    pub any_model: bool,
}

/// What `install` did or (dry run) would do.
#[derive(Debug, Default)]
pub struct Report {
    pub lines: Vec<String>,
    /// The image was written (OOS armed for the next power-on).
    pub written: bool,
    /// `recovery` already holds the package's image: nothing to do.
    pub unchanged: bool,
}

impl Report {
    fn say(&mut self, l: impl Into<String>) {
        self.lines.push(l.into());
    }
}

fn io(what: &str) -> impl Fn(std::io::Error) -> String + '_ {
    move |e| format!("{}: {}", what, e)
}

mod locking;
pub use locking::{lock, Lock};

/// `State::event` while the rollback slot is being written.
const SLOT_WRITING: &str = "writing the rollback slot";

/// The build id of the running image (/etc/oos-build, the same text `pack --build` puts into the package).
pub fn running_build() -> Option<String> {
    std::fs::read_to_string("/etc/oos-build").ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

mod install;
use install::write_trial;
pub use install::{install, install_direct};
pub use kexec::{kexec_missed, kexec_stage, BootTrees, Staged};

fn check_slot(card: &mut Card, st: &State) -> Result<(), String> {
    // An interrupted write can leave data without an image header.
    if !st.slot_incomplete
        && card.header(Region::Slot).map_err(io("reading the rollback slot"))?.is_none()
        && !card.region_empty(Region::Slot).map_err(io("reading the rollback slot"))?
    {
        return Err("the rollback slot (backup + 32 MiB) holds data that is not an OOS image: nothing written, inspect it first".into());
    }
    Ok(())
}

fn slot_holds(card: &mut Card, e: &Entry) -> Result<bool, String> {
    let (_, sha) = card.read_image(Region::Slot, e.len as usize).map_err(io("reading the rollback slot"))?;
    Ok(sha == e.sha256)
}

/// Write the rollback image back to `recovery` (checked against its SHA-256 first) and arm OOS. `expect`: only when
/// the state still names this image as current (the watcher's view at its start).
pub fn rollback(
    card: &mut Card,
    state_path: &Path,
    reason: &str,
    execute: bool,
    expect: Option<[u8; 32]>,
) -> Result<Report, String> {
    let mut r = Report::default();
    card.check_layout()?;
    let _l = if execute { Some(lock(state_path)?) } else { None };
    let mut st = State::load(state_path).map_err(io("reading the update state"))?;
    if let Some(x) = expect {
        if st.current.as_ref().map(|c| c.sha256) != Some(x) {
            return Err("the update state changed meanwhile (another install or rollback): not rolled back".into());
        }
    }
    let Some(prev) = st.previous.clone() else { return Err(st.missing_rollback()) };
    let (b, sha) = card.read_image(Region::Slot, prev.len as usize).map_err(io("reading the rollback slot"))?;
    if sha != prev.sha256 {
        return Err(format!(
            "the rollback slot does not hold build {} any more (SHA-256 differs): nothing written",
            short(&prev.build)
        ));
    }
    let id = check_image(&b)?.id;
    // the card decides, not the state: a cut write may have left something else there
    let (_, in_recovery) = card.read_image(Region::Recovery, prev.len as usize).map_err(io("reading recovery"))?;
    if in_recovery == prev.sha256 {
        r.say(format!("recovery already holds build {}", short(&prev.build)));
        if execute && (st.trial.is_some() || st.current.as_ref().map(|c| c.sha256) != Some(prev.sha256)) {
            st.abandon_card_trial();
            st.track_recovery(prev, id);
            st.trial = None;
            st.save(state_path).map_err(io("writing the update state"))?;
        }
        return Ok(r);
    }
    let from = st.current.as_ref().map_or("an untracked image".to_string(), |c| short(&c.build).to_string());
    r.say(format!("write build {} from the rollback slot back to recovery (replaces {})", short(&prev.build), from));
    if !execute {
        r.say("dry run: nothing written (--execute)");
        return Ok(r);
    }
    // the state keeps naming the image and its trial until the write is done: a cut write stays a rollback to repeat
    st.abandon_card_trial();
    st.event = format!("rolling back from {} to {}", from, short(&prev.build));
    st.save(state_path).map_err(io("writing the update state (nothing written to the card)"))?;
    card.disarm_oos()?;
    card.write_image(Region::Recovery, &b)?;
    r.written = true;
    st.event = format!("rolled back from {} to {}: {}", from, short(&prev.build), reason);
    st.track_recovery(prev.clone(), id);
    st.trial = None;
    st.save(state_path).map_err(io("writing the update state"))?;
    r.say("recovery written and read back");
    r.lines.extend(st.sd_fallback().map(str::to_string));
    match card.arm_oos() {
        Ok(_) => r.say(text::restored(&prev.build)),
        Err(e) => r.say(format!("WARNING: {}", e)),
    }
    Ok(r)
}

/// Persist mode after a good start: arm OOS, under the lock and only with a valid image in `recovery` and no trial. Ok(true)
/// when armed now.
pub fn arm_checked(card: &mut Card, state_path: &Path) -> Result<bool, String> {
    let _l = lock(state_path)?;
    let st = State::load(state_path).map_err(io("reading the update state"))?;
    if st.trial.is_some() {
        return Err("an image is on trial: not armed".into());
    }
    let h = card.header(Region::Recovery).map_err(io("reading recovery"))?.ok_or("no image in recovery: not armed")?;
    let (b, _) = card.read_image(Region::Recovery, h.len).map_err(io("reading recovery"))?;
    check_image(&b)?;
    card.arm_oos()
}

/// Selects OOS for the next power-on by hand (`oos-boot-oos-again`): only an image in `recovery` the loader takes, and
/// never while an install or rollback runs. Unlike `arm_checked` it arms a trial image too: its second start is what
/// rolls it back.
pub fn arm_by_hand(card: &mut Card, state_path: &Path) -> Result<bool, String> {
    let _l = lock(state_path)?;
    let h = card.header(Region::Recovery).map_err(io("reading recovery"))?.ok_or("no image in recovery: not armed")?;
    let (b, _) = card.read_image(Region::Recovery, h.len).map_err(io("reading recovery"))?;
    check_image(&b).map_err(|e| format!("recovery: {}: not armed", e))?;
    card.arm_oos()
}

/// An install or rollback holds the update lock right now (also one of another process: the start-up watcher).
pub fn busy(state_path: &Path) -> bool {
    let Ok(f) = std::fs::OpenOptions::new().write(true).open(state_path.with_extension("lock")) else { return false };
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        // SAFETY: the lock file owns this descriptor for the duration of flock.
        if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            return true;
        }
    }
    drop(f);
    false
}

/// What `boot_started` found.
#[derive(Clone, Debug, PartialEq)]
pub enum Boot {
    /// no image on trial
    NoTrial,
    /// the trial image started, this many times now
    Trial(u32),
    /// the trial image started a second time without confirmation: the caller rolls back
    Rollback,
    /// the running image is not the one on trial (recovery written by hand): the trial was dropped
    Dropped(String),
}

/// Early in rcS: count this start of a trial image. `running`: the build id of the running image; the count only
/// applies when it is the image on trial.
pub fn boot_started(state_path: &Path, running: Option<&str>) -> Result<Boot, String> {
    boot_started_with_card(state_path, running, crate::bootcard::Status::default())
}

/// Count an ordinary trial start and record its card observation in the same state save.
pub fn boot_started_with_card(
    state_path: &Path,
    running: Option<&str>,
    status: crate::bootcard::Status,
) -> Result<Boot, String> {
    let _l = lock(state_path)?;
    let mut st = State::load(state_path).map_err(io("reading the update state"))?;
    let restart = read_restart_mark(state_path).map_err(io("reading the kexec restart guard"))?;
    // Resolve a missed trial before timing from a different running image can reset its miss.
    let boot = st.count_trial_start(running);
    let mut changed = false;
    if let Some(run) = running.map(str::trim).filter(|run| !run.is_empty() && !status.is_kexec()) {
        let warm = restart.as_deref().is_some_and(|mark| mark.trim() == run);
        let trial = st.trial.is_some()
            && !st.recovery.writing
            && st.current.as_ref().is_some_and(|current| run == current.build.trim());
        changed = st.sd.observe(status, warm, trial);
    }
    if changed || boot != Boot::NoTrial {
        st.save(state_path).map_err(io("writing the update state"))?;
    }
    if restart.is_some() {
        restart_mark::remove_restart_mark(state_path).map_err(io("removing the kexec restart guard"))?;
    }
    Ok(boot)
}

/// Boot mode (persist = arm OOS after every confirmed start).
pub fn set_mode(state_path: &Path, persist: bool) -> Result<(), String> {
    set_mode_with(state_path, persist, lock)
}

fn set_mode_with(
    state_path: &Path,
    persist: bool,
    acquire: impl FnOnce(&Path) -> Result<Lock, String>,
) -> Result<(), String> {
    let _l = acquire(state_path)?;
    let mut st = State::load(state_path).map_err(io("reading the update state"))?;
    st.persist = persist;
    st.save(state_path).map_err(io("writing the update state"))
}

/// What the watcher sees after `elapsed` seconds.
#[derive(Clone, Copy, Debug, Default)]
pub struct Health {
    pub elapsed: f64,
    /// Times oos-ui was (re)started after the first.
    pub ui_restarts: u32,
    /// Seconds the current oos-ui process has been running (0 = none).
    pub ui_up: f64,
    /// The start-up sequence wrote `end` or `abort` (or there is none: no acquisition hardware in this image).
    pub startup_done: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    Wait,
    Good,
    Bad(String),
}

pub const UI_STABLE_S: f64 = 20.0;
pub const UI_RESTARTS_BAD: u32 = 3;
pub const START_TIMEOUT_S: f64 = 180.0;

/// The trial rule: the image's software must run. oos-ui up 20 s without crashing and the start-up finished (`end`
/// or `abort`) is good at once; a start-up that hangs while oos-ui runs stable is good at the timeout too (the
/// screen names the stage, Utility > Roll back update is there): hardware trouble must not roll a good build back.
/// Bad: 3 oos-ui crashes, or no stable oos-ui by the timeout.
pub fn judge(h: &Health) -> Verdict {
    if h.ui_restarts >= UI_RESTARTS_BAD {
        return Verdict::Bad(format!("oos-ui crashed {} times", h.ui_restarts));
    }
    if h.ui_up >= UI_STABLE_S && (h.startup_done || h.elapsed >= START_TIMEOUT_S) {
        return Verdict::Good;
    }
    if h.elapsed >= START_TIMEOUT_S {
        return Verdict::Bad(format!("oos-ui did not run stable within {} s", START_TIMEOUT_S));
    }
    Verdict::Wait
}

#[cfg(test)]
mod tests;
