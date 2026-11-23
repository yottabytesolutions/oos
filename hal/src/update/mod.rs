// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Signed OOS updates with rollback (docs/update.md).
//!
//! - `pkg`: the signed package `.oosupd` (Ed25519 over a text header that names the image's SHA-256).
//! - `card`: the card layout, the image regions (`recovery` and the rollback slot in `backup`) and the boot magic.
//! - `first`: the first install from the running stock system, without opening the scope.
//! - `kexec`: the trial start of a new image by kexec, before anything is written.
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
use hash::hex;
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
pub struct State {
    /// The image in `recovery` (None: put there by hand, not by an update).
    pub current: Option<Entry>,
    /// The image in the rollback slot.
    pub previous: Option<Entry>,
    /// Some(n): `current` is on trial, started n times without being confirmed.
    pub trial: Option<u32>,
    /// Arm OOS again after every confirmed start.
    pub persist: bool,
    /// Suppress an SDR104 retry after an unconfirmed boot.
    sd_failed: bool,
    /// A same-build DTB trial must actually start before it can be confirmed.
    sd_trial: Option<sd::Mode>,
    /// The build whose kexec start is under way (`kexec_stage`): nothing of it is on the card yet.
    pub kexec: Option<String>,
    /// Last thing that happened, one line.
    pub event: String,
}

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

/// Verify `package`, keep the image in `recovery` as the rollback image (when it is a confirmed or hand-installed
/// one), write the new image, read it back, mark it on trial and arm OOS for the next power-on.
pub fn install(
    card: &mut Card,
    state_path: &Path,
    package: &[u8],
    keys: &[[u8; 32]],
    opt: &InstallOpt,
) -> Result<Report, String> {
    let (h, img, mut r) = models::checked_package(package, keys, opt)?;
    let m = &h.manifest;
    let info = check_image(img)?;
    if info.len != img.len() {
        return Err(format!("image is {} bytes, its sections end at {}", img.len(), info.len));
    }
    r.say(format!("image: '{}', loader id {} ok", info.name, &hex(&info.id)[..16]));
    card.check_layout()?;
    let _l = if opt.execute { Some(lock(state_path)?) } else { None };
    let mut st = State::load(state_path).map_err(io("reading the update state"))?;
    let mode = sd::configured(state_path, &st)?;
    let image = sd::image(img, mode)?;
    let img = image.as_slice();
    let new = Entry { build: m.build.clone(), sha256: hash::sha256(img), len: img.len() as u64 };

    if let Some(cur) = st.current.as_ref().filter(|c| !c.build.starts_with("untracked")) {
        if cur.build_key() > new.build_key() && !opt.downgrade {
            return Err(format!(
                "build {} is older than the installed {} (give --downgrade to install it anyway)",
                short(&new.build),
                short(&cur.build)
            ));
        }
    }
    // what recovery holds now
    let cur_hdr = card.header(Region::Recovery).map_err(io("reading recovery"))?;
    let mut cur_img = None;
    if let Some(hdr) = &cur_hdr {
        let (b, sha) = card.read_image(Region::Recovery, hdr.len).map_err(io("reading recovery"))?;
        if sha == new.sha256 {
            if !opt.force {
                r.say(format!(
                    "recovery already holds build {}: nothing to do (--force writes it again)",
                    short(&new.build)
                ));
                r.unchanged = true;
                return Ok(r);
            }
            // the same image again: never copied into the slot (that would replace the last good one)
        } else {
            match check_image(&b) {
                Ok(_) => cur_img = Some((b, sha)),
                Err(e) => r.say(format!("recovery holds a damaged image ({}): not kept", e)),
            }
        }
    } else {
        r.say("recovery holds no OOS image: nothing to keep");
    }

    // keep it as the rollback image? Only a confirmed or hand-installed one: an image still on trial is not known
    // to work, the slot keeps the last good one then.
    let mut save: Option<(Vec<u8>, Entry)> = None;
    match (&cur_img, st.trial) {
        (Some((b, sha)), None) => {
            let saved = sd::image(b, sd::Mode::Hs)?;
            let saved_sha = hash::sha256(&saved);
            let e = Entry {
                build: match &st.current {
                    Some(c) if c.sha256 == *sha => c.build.clone(),
                    _ => format!("untracked-{}", &hex(sha)[..12]),
                },
                sha256: saved_sha,
                len: saved.len() as u64,
            };
            if st.previous.as_ref().is_some_and(|p| p.sha256 == saved_sha) && slot_holds(card, &e)? {
                r.say(format!("rollback slot already holds build {}", short(&e.build)));
            } else {
                r.say(format!("keep build {} ({} bytes) in the {}", short(&e.build), e.len, Region::Slot.name()));
                save = Some((saved, e));
            }
        }
        (Some(_), Some(n)) => r.say(format!(
            "the image in recovery is on trial ({} starts, not confirmed): not kept, the slot keeps {}",
            n,
            st.previous.as_ref().map_or("nothing".into(), |p| short(&p.build).to_string())
        )),
        (None, _) => {}
    }
    if save.is_some() {
        check_slot(card, &st)?;
    }
    if save.is_none() && st.previous.is_none() {
        if mode == sd::Mode::Sdr104 {
            return Err("SDR104 needs a confirmed HS rollback image: nothing written".into());
        }
        r.say("WARNING: no rollback image; a failed start falls back to stock Android only");
    }
    r.say(format!("write build {} to recovery, on trial; arm OOS for the next power-on", short(&new.build)));
    if !opt.execute {
        r.say("dry run: nothing written (--execute)");
        return Ok(r);
    }

    // the state file must be writable before the card is touched (a full /oos would leave the new image untracked)
    st.save(state_path).map_err(io("writing the update state (nothing written to the card)"))?;
    if let Some((b, e)) = save {
        state::write_slot(card, state_path, &b, e, &mut st)?;
        r.say("rollback slot written and read back");
    }
    st.sd_trial = (mode == sd::Mode::Sdr104).then_some(mode);
    if sd::requested(state_path)? == sd::Mode::Hs {
        st.sd_failed = false;
    }
    write_trial(card, state_path, img, &new.build, &mut st, &mut r)?;
    Ok(r)
}

fn write_trial(
    card: &mut Card,
    state_path: &Path,
    img: &[u8],
    build: &str,
    st: &mut State,
    r: &mut Report,
) -> Result<(), String> {
    let old = st.current.clone();
    // stock for the next power-on while recovery is not a valid image (`Card::disarm_oos`); armed again below
    card.disarm_oos()?;
    // recorded before the write: a failed or cut write leaves a state that names the new image on trial (a rollback or a
    // new install repairs it)
    st.current = Some(Entry { build: build.into(), sha256: hash::sha256(img), len: img.len() as u64 });
    st.trial = Some(0);
    st.event = format!("writing build {}", short(build));
    st.save(state_path).map_err(io("writing the update state"))?;
    card.write_image(Region::Recovery, img)?;
    r.written = true;
    // this was the install a kexec start was for (or one that replaces it); a cut one keeps the mark, which the next
    // ordinary start reports
    st.kexec = None;
    st.event = format!(
        "installed {} over {}",
        short(build),
        old.as_ref().map_or("an untracked image".into(), |o| short(&o.build).to_string())
    );
    st.save(state_path).map_err(io("writing the update state"))?;
    r.say("recovery written and read back");
    match card.arm_oos() {
        Ok(_) => r.say(text::installed(build)),
        Err(e) => r.say(format!("WARNING: {}; start OOS with oos-boot-oos-again", e)),
    }
    Ok(())
}

/// What a kexec start needs: kernel, initramfs (with the package, the key that verified it and the device tree section
/// the start runs with), the build; and for an image with another device tree, the tree to start it with (the new one
/// with the loader's changes, `fdt::with_loader_changes`), which `kexec::load_with_tree` takes.
#[derive(Debug)]
pub struct Staged {
    pub kernel: Vec<u8>,
    pub initrd: Vec<u8>,
    pub build: String,
    pub tree: Option<Vec<u8>>,
}

/// The first half of an install by kexec start (module `kexec`): every check of `install` (nothing written), the device
/// tree, then the state names the build. The boot selection stays: `recovery` still holds the running build, so a
/// power-cycle after a hang starts that one (or stock in one-shot mode). The caller loads and starts the kernel; the
/// second half is `install` run by the started image itself (`oos-update boot`). None: nothing to do.
///
/// `booted_dt`: the device tree section of the image the loader started (`kexec::BOOTED_DT`, saved by `oos-update
/// boot`); without it only an image in `recovery` that is the running build (`running`) tells. `running_tree`: the tree
/// the running kernel got (`/sys/firmware/fdt`), given when a kexec start with another tree can be made
/// (`kexec::TOOL`); without it an image with another tree is refused.
pub fn kexec_stage(
    card: &mut Card,
    state_path: &Path,
    package: &[u8],
    keys: &[[u8; 32]],
    opt: &InstallOpt,
    booted_dt: Option<&[u8]>,
    running: Option<&str>,
    running_tree: Option<&[u8]>,
) -> Result<(Report, Option<Staged>), String> {
    let mut r = install(card, state_path, package, keys, &InstallOpt { execute: false, ..*opt })?;
    if r.unchanged {
        return Ok((r, None));
    }
    // (the dry run's own last line says nothing was written: true, but the kexec start follows)
    r.lines.pop();
    let (h, img) = pkg::verify(package, keys)?;
    let m = &h.manifest;
    if !kexec::announced(m) {
        return Err(format!("build {} does not finish a kexec start (its package has no 'trial kexec' line): install it with --no-kexec", short(&m.build)));
    }
    let selected =
        sd::image(img, sd::configured(state_path, &State::load(state_path).map_err(io("reading update state"))?)?)?;
    let (kernel, ramdisk, dt) = kexec::parts(&selected)?;
    // the tree the running kernel got from the loader, which the kexec start keeps
    let booted: Vec<u8> = match booted_dt {
        Some(b) => b.to_vec(),
        None => {
            let st = State::load(state_path).map_err(io("reading the update state"))?;
            let cur_is_running = st.current.as_ref().zip(running).is_some_and(|(c, run)| c.build.trim() == run.trim());
            if !cur_is_running {
                return Err(format!("the device tree this scope started with is not known ({} missing, recovery may hold another build): power-cycle once, or install with --new-dt", kexec::BOOTED_DT));
            }
            let cur = card
                .header(Region::Recovery)
                .map_err(io("reading recovery"))?
                .ok_or("recovery holds no OOS image: no device tree to compare with")?;
            let (cur_img, _) = card.read_image(Region::Recovery, cur.len).map_err(io("reading recovery"))?;
            kexec::parts(&cur_img).map_err(|e| format!("recovery: {}: no device tree to compare with", e))?.2.to_vec()
        }
    };
    let tree = sd::trial_tree(dt, &booted, running_tree, &mut r)?;
    let key = keys.iter().find(|k| pkg::key_id(k) == m.key).ok_or("no key for the package")?;
    // the started image keeps the tree section it runs with (for its own updates later): the new one, when it brings one
    let staged = Staged {
        kernel: kernel.to_vec(),
        initrd: kexec::initrd(ramdisk, package, key, dt),
        build: m.build.clone(),
        tree,
    };
    if !opt.execute {
        r.say(format!(
            "start build {} by kexec; it installs itself once it runs (dry run: --execute)",
            short(&staged.build)
        ));
        return Ok((r, None));
    }
    let _l = lock(state_path)?;
    let mut st = State::load(state_path).map_err(io("reading the update state"))?;
    st.sd_trial = (sd::configured(state_path, &st)? == sd::Mode::Sdr104).then_some(sd::Mode::Sdr104);
    st.kexec = Some(staged.build.clone());
    st.event = format!("starting build {} by kexec for its trial", short(&staged.build));
    st.save(state_path).map_err(io("writing the update state (nothing written to the card)"))?;
    r.say(format!("starting build {} by kexec: it installs itself once it runs", short(&staged.build)));
    Ok((r, Some(staged)))
}

/// The kexec start is over without the install it was for (the load or the start failed, or the started image could
/// not install itself): forget it, with `event` as the last event.
pub fn kexec_clear(state_path: &Path, event: &str) -> Result<(), String> {
    let _l = lock(state_path)?;
    let mut st = State::load(state_path).map_err(io("reading the update state"))?;
    if st.kexec.take().is_none() {
        return Ok(());
    }
    st.sd_trial = None;
    st.event = event.into();
    st.save(state_path).map_err(io("writing the update state"))
}

/// Early in rcS of an ordinary start: a kexec start that was under way did not finish its install (the new kernel or its
/// start-up hung, or the power went off). Returns the message, once.
pub fn kexec_missed(state_path: &Path, kexec_start: bool) -> Result<Option<String>, String> {
    if kexec_start {
        return Ok(None);
    }
    let _l = lock(state_path)?;
    let mut st = State::load(state_path).map_err(io("reading the update state"))?;
    let Some(b) = st.kexec.take() else { return Ok(None) };
    let fallback = st.missed_sd_trial();
    let msg = format!("Update {} did not finish when started by kexec (its kernel or start-up hung, or the power went off): not installed{}", short(&b), fallback);
    st.event = msg.clone();
    st.save(state_path).map_err(io("writing the update state"))?;
    Ok(Some(msg))
}

fn check_slot(card: &mut Card, st: &State) -> Result<(), String> {
    let slot_hdr = card.header(Region::Slot).map_err(io("reading the rollback slot"))?;
    // (a write of the slot cut by a power loss can leave data without its header: then it is ours to overwrite)
    if slot_hdr.is_none()
        && st.event != SLOT_WRITING
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
    let prev = st.previous.clone().ok_or("no rollback image (nothing was kept by an update)")?;
    let (b, sha) = card.read_image(Region::Slot, prev.len as usize).map_err(io("reading the rollback slot"))?;
    if sha != prev.sha256 {
        return Err(format!(
            "the rollback slot does not hold build {} any more (SHA-256 differs): nothing written",
            short(&prev.build)
        ));
    }
    check_image(&b)?;
    // the card decides, not the state: a cut write may have left something else there
    let (_, in_recovery) = card.read_image(Region::Recovery, prev.len as usize).map_err(io("reading recovery"))?;
    if in_recovery == prev.sha256 {
        r.say(format!("recovery already holds build {}", short(&prev.build)));
        if execute && (st.trial.is_some() || st.current.as_ref().map(|c| c.sha256) != Some(prev.sha256)) {
            st.current = Some(prev.clone());
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
    st.fail_sd_trial();
    st.event = format!("rolling back from {} to {}", from, short(&prev.build));
    st.save(state_path).map_err(io("writing the update state (nothing written to the card)"))?;
    card.disarm_oos()?;
    card.write_image(Region::Recovery, &b)?;
    r.written = true;
    st.current = Some(prev.clone());
    st.trial = None;
    st.event = format!("rolled back from {} to {}: {}", from, short(&prev.build), reason);
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
    let _l = lock(state_path)?;
    let mut st = State::load(state_path).map_err(io("reading the update state"))?;
    let Some(n) = st.trial else { return Ok(Boot::NoTrial) };
    let cur = st.current.as_ref().map(|c| c.build.clone()).unwrap_or_default();
    if let Some(run) = running {
        if run.trim() != cur.trim() {
            let msg = format!(
                "running build {} is not the one on trial ({}): trial dropped, recovery treated as hand-installed",
                short(run),
                short(&cur)
            );
            st.fail_sd_trial();
            st.trial = None;
            st.current = None;
            st.event = msg.clone();
            st.save(state_path).map_err(io("writing the update state"))?;
            return Ok(Boot::Dropped(msg));
        }
    }
    st.trial = Some(n + 1);
    st.save(state_path).map_err(io("writing the update state"))?;
    Ok(if n + 1 >= 2 && st.previous.is_some() { Boot::Rollback } else { Boot::Trial(n + 1) })
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
