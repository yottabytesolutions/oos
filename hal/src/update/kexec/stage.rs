// SPDX-License-Identifier: GPL-2.0-or-later

use super::{announced, announced_staged, initrd, parts, staged_initrd, Flags, Receive, BOOTED_DT};
use crate::update::{
    install::{check_can_start, prepare, validate_image},
    io, lock, models, pkg, sd, short, Card, Entry, InstallOpt, Region, Report, State,
};
use std::path::Path;

/// Kernel, initramfs and optional loader-adjusted tree prepared for a kexec start.
#[derive(Debug)]
pub struct Staged {
    pub kernel: Vec<u8>,
    pub initrd: Vec<u8>,
    pub build: String,
    pub tree: Option<Vec<u8>>,
    source: Receive,
}

impl Staged {
    /// Command line for this image's handoff format and install flags.
    pub fn cmdline(&self, running: &str) -> String {
        self.source.cmdline(running)
    }

    /// Bytes counted by the kexec payload guard, including an explicitly supplied tree.
    pub fn payload_size(&self) -> usize {
        self.kernel.len() + self.initrd.len() + self.tree.as_ref().map_or(0, Vec::len)
    }
}

/// Tree context for a kexec start. The installed tree can identify the booted tree only
/// when the running build matches recovery. A changed-tree start also needs the live tree.
#[derive(Debug)]
pub struct BootTrees<'a> {
    booted: Option<&'a [u8]>,
    running_build: Option<&'a str>,
    live: Option<&'a [u8]>,
}

impl<'a> BootTrees<'a> {
    pub fn new(booted: Option<&'a [u8]>, running_build: Option<&'a str>, live: Option<&'a [u8]>) -> Self {
        Self { booted, running_build, live }
    }
}

/// Verify the install, prepare the boot payload and record the pending kexec start.
/// A staged-capable package uses the slot while recovery and boot selection stay intact.
/// The candidate installs itself after it reaches rcS. Returns None for a dry run or no change.
pub fn kexec_stage(
    card: &mut Card,
    state_path: &Path,
    package: &[u8],
    keys: &[[u8; 32]],
    opt: &InstallOpt,
    trees: BootTrees<'_>,
) -> Result<(Report, Option<Staged>), String> {
    let (header, signed_image, mut report) = models::checked_package(package, keys, opt)?;
    validate_image(card, signed_image, &mut report)?;
    let manifest = &header.manifest;
    let to_slot = announced_staged(manifest);
    let _lock = if opt.execute { Some(lock(state_path)?) } else { None };
    let mut state = State::load(state_path).map_err(io("reading the update state"))?;
    check_can_start(&state)?;
    if to_slot {
        // The candidate displaces any previously kept rollback image. Validate the install on that basis.
        state.previous = None;
    }
    let mode = sd::configured(state_path, &state)?;
    let Some(plan) = prepare(card, signed_image, &manifest.build, &state, mode, opt, &mut report)? else {
        return Ok((report, None));
    };
    if !announced(manifest) {
        return Err(format!("build {} does not finish a kexec start (its package does not announce a kexec receiver): install it with --no-kexec", short(&manifest.build)));
    }
    if to_slot && plan.rollback.is_none() {
        return Err("staged install needs a different, valid image in recovery to keep for rollback".into());
    }
    drop(plan.rollback);
    let (kernel, ramdisk, dt) = parts(&plan.image)?;
    let booted = booted_tree(card, &state, trees.booted, trees.running_build)?;
    let tree = sd::trial_tree(dt, &booted, trees.live, &mut report)?;
    let build = short(&manifest.build);
    if !opt.execute {
        if to_slot {
            report.say(format!("stage build {build} in the slot, replacing any rollback image"));
        }
        report.say(format!("start build {build} by kexec; it installs itself once it runs (dry run: --execute)"));
        return Ok((report, None));
    }
    let key = keys.iter().find(|k| pkg::key_id(k) == manifest.key).ok_or("no key for the package")?;
    let flags = Flags { downgrade: opt.downgrade, force: opt.force, any_model: opt.any_model };
    let (source, initrd) = if to_slot {
        (Receive::Staged(flags), staged_initrd(ramdisk, &package[..header.data_offset], key, dt))
    } else {
        (Receive::Legacy(flags), initrd(ramdisk, package, key, dt))
    };
    let start = Staged { kernel: kernel.to_vec(), initrd, build: manifest.build.clone(), tree, source };
    if to_slot {
        let tree_size = start.tree.as_ref().map_or(0, Vec::len);
        super::budget::check(start.kernel.len(), start.initrd.len(), tree_size)?;
        let entry = Entry { build: start.build.clone(), sha256: manifest.sha256, len: manifest.size };
        stage_image(card, state_path, &mut state, entry, signed_image)?;
        report.say("candidate written to the slot and read back");
    }
    record_start(state_path, &mut state, &start, mode)?;
    report.say(&state.event);
    Ok((report, Some(start)))
}

fn booted_tree(
    card: &mut Card,
    state: &State,
    booted: Option<&[u8]>,
    running: Option<&str>,
) -> Result<Vec<u8>, String> {
    if let Some(tree) = booted {
        return Ok(tree.to_vec());
    }
    let cur_is_running = state.current.as_ref().zip(running).is_some_and(|(c, run)| c.build.trim() == run.trim());
    if !cur_is_running {
        return Err(format!("the device tree this scope started with is not known ({BOOTED_DT} missing, recovery may hold another build): power-cycle once, or install with --new-dt"));
    }
    let cur = card
        .header(Region::Recovery)
        .map_err(io("reading recovery"))?
        .ok_or("recovery holds no OOS image: no device tree to compare with")?;
    let (image, _) = card.read_image(Region::Recovery, cur.len).map_err(io("reading recovery"))?;
    parts(&image).map_err(|e| format!("recovery: {e}: no device tree to compare with")).map(|(_, _, dt)| dt.to_vec())
}

pub(in crate::update) fn stage_image(
    card: &mut Card,
    state_path: &Path,
    state: &mut State,
    entry: Entry,
    image: &[u8],
) -> Result<(), String> {
    let build = short(&entry.build).to_owned();
    state.previous = None;
    state.staged = Some(entry);
    state.slot_incomplete = true;
    state.event = format!("staging build {build}");
    state.save(state_path).map_err(io("writing the update state before staging"))?;
    if let Err(error) = card.write_image(Region::Slot, image) {
        state.staged = None;
        state.event = format!("staging build {build} failed ({error}), no rollback image");
        state.save(state_path).map_err(|e| format!("{}. Could not record the failure ({e})", state.event))?;
        return Err(state.event.clone());
    }
    Ok(())
}

fn record_start(state_path: &Path, state: &mut State, start: &Staged, mode: sd::Mode) -> Result<(), String> {
    state.sd_trial = (mode == sd::Mode::Sdr104).then_some(mode);
    state.kexec = Some(start.build.clone());
    let payload = start.payload_size();
    state.kexec_payload = Some(payload);
    state.event = format!("starting build {} by kexec for its trial ({payload} bytes payload)", short(&start.build));
    state.save(state_path).map_err(io("recording the kexec start"))
}

/// Report a missed kexec start or unused staged candidate once at the next ordinary start.
pub fn kexec_missed(state_path: &Path, kexec_start: bool) -> Result<Option<String>, String> {
    if kexec_start {
        return Ok(None);
    }
    let _lock = lock(state_path)?;
    let mut state = State::load(state_path).map_err(io("reading the update state"))?;
    let candidate = state.staged.take();
    let message = if let Some(build) = state.kexec.take() {
        let fallback = state.missed_sd_trial();
        let payload = state.take_kexec_payload();
        format!("Update {} did not finish when started by kexec (its kernel or start-up hung, or the power went off): not installed{fallback}{payload}", short(&build))
    } else if let Some(candidate) = candidate {
        format!("Staged update {} was never started: the next install will overwrite its slot", short(&candidate.build))
    } else {
        return Ok(None);
    };
    // Report the missed start independently of slot ownership, which survives until a verified rollback is written.
    state.event = message.clone();
    state.save(state_path).map_err(io("writing the update state"))?;
    Ok(Some(message))
}
