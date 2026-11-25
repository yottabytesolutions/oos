// SPDX-License-Identifier: GPL-2.0-or-later

//! Device-local boot card mode. Install applies it after package verification.

use super::{card, fdt, hash, io, kexec, Card, Entry, Report, State};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Mode {
    #[default]
    Hs,
    Sdr104,
}

impl Mode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hs => "hs",
            Self::Sdr104 => "sdr104",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Hs => "HS",
            Self::Sdr104 => "SDR104",
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut selected = "hs".to_string();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            if key.trim() == "sd.mode" {
                let value = value.replace("\t#", " #");
                selected = value.split_once(" #").map_or(value.as_str(), |(v, _)| v).trim().into();
            }
        }
        match selected.as_str() {
            "hs" => Ok(Self::Hs),
            "sdr104" => Ok(Self::Sdr104),
            _ => Err("sd.mode must be hs or sdr104".into()),
        }
    }
}

/// Record a card observation from the running kexec candidate when it matches the pending handoff.
pub fn observe_handoff(state_path: &Path, running: &str, status: crate::bootcard::Status) -> Result<(), String> {
    let _lock = super::lock(state_path)?;
    let mut state = State::load(state_path).map_err(io("reading update state"))?;
    if status.is_kexec()
        && !running.trim().is_empty()
        && state.kexec.as_deref().map(str::trim) == Some(running.trim())
        && state.observe_sd_trial(status)
    {
        state.save(state_path).map_err(io("recording the SDR104 handoff observation"))?;
    }
    Ok(())
}

fn config_path(state_path: &Path) -> Result<std::path::PathBuf, String> {
    let data = state_path.parent().and_then(Path::parent).ok_or("update state has no data directory")?;
    Ok(data.join("system.conf"))
}

fn read_config(path: &Path) -> std::io::Result<String> {
    match std::fs::read_to_string(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        result => result,
    }
}

pub(super) fn requested(state_path: &Path) -> Result<Mode, String> {
    let text = read_config(&config_path(state_path)?).map_err(io("reading device system.conf"))?;
    Mode::parse(&text)
}

/// Serialize read-modify-replace of system.conf by the updater and the panel.
pub fn lock_config(path: &Path) -> Result<super::Lock, String> {
    super::locking::lock_with(path, true)
}

fn save_preference(state_path: &Path, mode: Mode) -> Result<(), String> {
    let path = config_path(state_path)?;
    let _lock = lock_config(&path)?;
    let text = read_config(&path).map_err(io("reading device system.conf"))?;
    let mut updated = String::new();
    for line in text.split_inclusive('\n') {
        if !line.split_once('=').is_some_and(|(key, _)| key.trim() == "sd.mode") {
            updated.push_str(line);
        }
    }
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push_str(&format!("sd.mode = {}\n", mode.as_str()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    crate::system::durable_replace(&path, &path.with_extension("sd.new"), updated.as_bytes(), &options)
        .map_err(io("saving boot card preference"))
}

pub(super) fn configured(state_path: &Path, state: &State) -> Result<Mode, String> {
    let mode = requested(state_path)?;
    Ok(if state.sd_failed && mode == Mode::Sdr104 { Mode::Hs } else { mode })
}

fn find_mut<'a>(node: &'a mut fdt::Node, name: &str) -> Option<&'a mut fdt::Node> {
    if node.name == name {
        return Some(node);
    }
    node.kids.iter_mut().find_map(|n| find_mut(n, name))
}

/// The property delta in rk3399-rigol-dho900-uhs.dts; all unrelated nodes stay intact.
pub fn patch_tree(blob: &[u8], mode: Mode) -> Result<Vec<u8>, String> {
    let mut tree = fdt::Fdt::parse(blob)?;
    let sd = find_mut(&mut tree.root, "mmc@fe320000").ok_or("device tree has no boot SD controller")?;
    let supply = sd.prop("vqmmc-supply").ok_or("boot SD has no vqmmc supply")?.to_vec();
    for key in ["sd-uhs-sdr50", "sd-uhs-sdr104"] {
        sd.props.retain(|(k, _)| k != key);
    }
    if mode == Mode::Sdr104 {
        sd.set("sd-uhs-sdr50", &[]);
        sd.set("sd-uhs-sdr104", &[]);
        sd.set("max-frequency", &150_000_000u32.to_be_bytes());
    }
    let rail = find_mut(&mut tree.root, "LDO_REG4").ok_or("device tree has no vccio_sd regulator")?;
    if rail.prop("regulator-name") != Some(b"vccio_sd\0") || rail.prop("phandle") != Some(supply.as_slice()) {
        return Err("boot SD vqmmc is not the vccio_sd regulator".into());
    }
    rail.props.retain(|(k, _)| k != "regulator-settling-time-down-us" && k != "regulator-init-microvolt");
    if mode == Mode::Sdr104 {
        rail.set("regulator-settling-time-down-us", &30_000u32.to_be_bytes());
    }
    Ok(tree.to_bytes())
}

/// Replace the single-tree RSCE and recompute the stock loader's Android image id.
pub fn image(image: &[u8], mode: Mode) -> Result<Vec<u8>, String> {
    let (kernel, ramdisk, second) = kexec::parts(image)?;
    let Some(rsce) = patched_second(second, mode)? else { return Ok(image.to_vec()) };
    let mut out = image[..2048].to_vec();
    out[24..28].copy_from_slice(&(rsce.len() as u32).to_le_bytes());
    let id = card::loader_id(&out, kernel, ramdisk, &rsce);
    out[576..596].copy_from_slice(&id);
    for section in [kernel, ramdisk, &rsce] {
        out.extend_from_slice(section);
        out.resize(out.len().div_ceil(2048) * 2048, 0);
    }
    card::check_image(&out)?;
    Ok(out)
}

/// The loader ID after applying the same SDR104 change as install, without copying the image.
pub(super) fn sdr104_loader_id(image: &[u8]) -> Option<[u8; 20]> {
    let (kernel, ramdisk, second) = kexec::parts(image).ok()?;
    let rsce = patched_second(second, Mode::Sdr104).ok()??;
    Some(card::loader_id(image, kernel, ramdisk, &rsce))
}

fn patched_second(second: &[u8], mode: Mode) -> Result<Option<Vec<u8>>, String> {
    // Older HS packages need no FDT edits. An SDR104 request always requires a patchable tree.
    if mode == Mode::Hs && !second.starts_with(b"RSCE") {
        return Ok(None);
    }
    let original = fdt::rsce_file(second, fdt::TREE)?;
    if mode == Mode::Hs {
        let mut tree = fdt::Fdt::parse(original)?;
        let enabled = find_mut(&mut tree.root, "mmc@fe320000")
            .is_some_and(|sd| sd.prop("sd-uhs-sdr50").is_some() || sd.prop("sd-uhs-sdr104").is_some());
        if !enabled {
            return Ok(None);
        }
    }
    let dtb = patch_tree(original, mode)?;
    if dtb == original {
        return Ok(None);
    }
    if second.get(8..12) != Some(&[1, 1, 1, 0]) || second.get(12..16) != Some(&1u32.to_le_bytes()) {
        return Err("SD mode patch requires a single-tree RSCE container".into());
    }
    let mut rsce = second.get(..1024).ok_or("SD mode patch requires a complete RSCE table")?.to_vec();
    rsce[772..776].copy_from_slice(&2u32.to_le_bytes());
    rsce[776..780].copy_from_slice(&(dtb.len() as u32).to_le_bytes());
    rsce.extend_from_slice(&dtb);
    rsce.resize(rsce.len().div_ceil(4) * 4, 0);
    Ok(Some(rsce))
}

/// Re-stage a confirmed active image for the next boot using the updater's verified write path.
pub fn restage(card: &mut Card, state_path: &Path, execute: bool) -> Result<Report, String> {
    restage_preference(card, state_path, None, execute)
}

/// Explain why an existing update blocks a boot card restage.
pub fn restage_blocker(state: &State) -> Option<String> {
    super::install::check_can_start(state).err()
}

/// Restage using an explicit preference, and save it to system.conf on execution.
pub fn restage_preference(
    card: &mut Card,
    state_path: &Path,
    preference: Option<Mode>,
    execute: bool,
) -> Result<Report, String> {
    card.check_layout()?;
    let _lock = execute.then(|| lock_for_restage(state_path)).transpose()?;
    let mut state = State::load(state_path).map_err(io("reading update state"))?;
    super::install::check_can_start(&state)?;
    let current = state.current.clone().ok_or("SD mode needs a tracked, confirmed active image")?;
    let (old, sha) = card.read_image(card::Region::Recovery, current.len as usize).map_err(io("reading recovery"))?;
    if sha != current.sha256 {
        return Err("active image SHA-256 differs from update state: nothing written".into());
    }
    let requested = match preference {
        Some(mode) => mode,
        None => requested(state_path)?,
    };
    if requested == Mode::Hs {
        state.sd_failed = false;
    }
    let mode = if state.sd_failed { Mode::Hs } else { requested };
    let next = image(&old, mode)?;
    let mut report = Report::default();
    report.say(format!("Boot card mode {} takes effect on the next OOS boot", mode.label()));
    report.lines.extend(state.sd_fallback().map(str::to_string));
    let changed = next != old;
    if changed {
        super::check_slot(card, &state)?;
    }
    if !execute {
        report.unchanged = !changed;
        report.say("dry run: nothing written (--execute)");
        return Ok(report);
    }
    let backup = if changed {
        let previous = image(&old, Mode::Hs)?;
        let entry = Entry { build: current.build, sha256: hash::sha256(&previous), len: previous.len() as u64 };
        Some((previous, entry))
    } else {
        None
    };
    if let Some(preference) = preference {
        save_preference(state_path, preference)?;
    }
    let result = if let Some(backup) = backup {
        write_mode_trial(card, state_path, &mut state, backup, &next, mode, &mut report)
    } else {
        report.unchanged = true;
        report.say("Recovery already has this boot card mode");
        state.save(state_path).map_err(io("writing update state"))
    };
    result.map_err(|error| match preference {
        Some(mode) => format!("Requested mode {} was saved. Restaging failed: {error}", mode.label()),
        None => error,
    })?;
    Ok(report)
}

fn lock_for_restage(state_path: &Path) -> Result<super::Lock, String> {
    super::locking::lock_with(state_path, true).map_err(|error| {
        format!("Cannot change boot card mode while the update state is busy or unavailable ({error})")
    })
}

fn write_mode_trial(
    card: &mut Card,
    state_path: &Path,
    state: &mut State,
    backup: (Vec<u8>, Entry),
    next: &[u8],
    mode: Mode,
    report: &mut Report,
) -> Result<(), String> {
    let (previous, entry) = backup;
    let build = entry.build.clone();
    super::state::write_slot(card, state_path, &previous, entry, state)?;
    state.sd_trial = Some(mode);
    super::write_trial(card, state_path, next, &build, state, report)
}

pub(super) fn trial_tree(
    dt: &[u8],
    booted: &[u8],
    running: Option<&[u8]>,
    report: &mut Report,
) -> Result<Option<Vec<u8>>, String> {
    let same = booted == dt;
    let Some(run) = running else {
        if !same {
            return Err("the image brings another device tree than the one this scope started with, and this build cannot start a kernel with another tree".into());
        }
        report.say("device tree: the same as the running one; kexec-tools is unavailable, using kexec_file_load");
        return Ok(None);
    };
    let merge_error = |error: String| {
        if same {
            format!("the unchanged device tree cannot be merged ({error}); kexec_file_load can place the initramfs over low-memory reservations. Try the image on a card you can take out, then use --new-dt for a direct install")
        } else {
            error
        }
    };
    let old_dt =
        fdt::rsce_file(booted, fdt::TREE).map_err(|e| merge_error(format!("the running image's device tree: {e}")))?;
    let new_dt = if same {
        old_dt
    } else {
        fdt::rsce_file(dt, fdt::TREE).map_err(|e| format!("the new image's device tree: {e}"))?
    };
    let (tree, n) =
        fdt::with_loader_changes(new_dt, old_dt, run).map_err(|e| merge_error(format!("the new device tree: {e}")))?;
    if same {
        report.say("device tree: the same as the running one; kexec-tools will load it above low memory");
    } else {
        report.say(format!("device tree: another one; the kexec start tries it, with the {} changes the loader made to the running one", n));
    }
    Ok(Some(tree))
}
