// SPDX-License-Identifier: GPL-2.0-or-later

//! Device-local boot card mode, applied only after authenticating the package.

use super::{card, fdt, hash, io, kexec, Card, Entry, Report, State};
use std::path::Path;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Mode {
    #[default]
    Hs,
    Sdr104,
}

impl Mode {
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

pub(super) fn requested(state_path: &Path) -> Result<Mode, String> {
    let data = state_path.parent().and_then(Path::parent).ok_or("update state has no data directory")?;
    let text = match std::fs::read_to_string(data.join("system.conf")) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("reading device system.conf: {}", e)),
    };
    Mode::parse(&text)
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
    // Older HS packages need no FDT edits. An SDR104 request always requires a patchable tree.
    if mode == Mode::Hs && !second.starts_with(b"RSCE") {
        return Ok(image.to_vec());
    }
    let original = fdt::rsce_file(second, fdt::TREE)?;
    if mode == Mode::Hs {
        let mut tree = fdt::Fdt::parse(original)?;
        let enabled = find_mut(&mut tree.root, "mmc@fe320000")
            .is_some_and(|sd| sd.prop("sd-uhs-sdr50").is_some() || sd.prop("sd-uhs-sdr104").is_some());
        if !enabled {
            return Ok(image.to_vec());
        }
    }
    let dtb = patch_tree(original, mode)?;
    if dtb == original {
        return Ok(image.to_vec());
    }
    if second.get(8..12) != Some(&[1, 1, 1, 0]) || second.get(12..16) != Some(&1u32.to_le_bytes()) {
        return Err("SD mode patch requires a single-tree RSCE container".into());
    }
    let mut rsce = second.get(..1024).ok_or("SD mode patch requires a complete RSCE table")?.to_vec();
    rsce[772..776].copy_from_slice(&2u32.to_le_bytes());
    rsce[776..780].copy_from_slice(&(dtb.len() as u32).to_le_bytes());
    rsce.extend_from_slice(&dtb);
    rsce.resize(rsce.len().div_ceil(4) * 4, 0);
    let mut out = image[..2048].to_vec();
    out[24..28].copy_from_slice(&(rsce.len() as u32).to_le_bytes());
    let id = hash::sha1(&[
        kernel,
        &(kernel.len() as u32).to_le_bytes(),
        ramdisk,
        &(ramdisk.len() as u32).to_le_bytes(),
        &rsce,
        &(rsce.len() as u32).to_le_bytes(),
        &out[32..576],
    ]);
    out[576..596].copy_from_slice(&id);
    for section in [kernel, ramdisk, &rsce] {
        out.extend_from_slice(section);
        out.resize(out.len().div_ceil(2048) * 2048, 0);
    }
    card::check_image(&out)?;
    Ok(out)
}

/// Re-stage a confirmed active image for the next boot using the updater's verified write path.
pub fn restage(card: &mut Card, state_path: &Path, execute: bool) -> Result<Report, String> {
    card.check_layout()?;
    let _lock = if execute { Some(super::lock(state_path)?) } else { None };
    let mut state = State::load(state_path).map_err(io("reading update state"))?;
    if state.trial.is_some() || state.kexec.is_some() {
        return Err("an update is on trial or being staged: SD mode left unchanged".into());
    }
    let current = state.current.clone().ok_or("SD mode needs a tracked, confirmed active image")?;
    let (old, sha) = card.read_image(card::Region::Recovery, current.len as usize).map_err(io("reading recovery"))?;
    if sha != current.sha256 {
        return Err("active image SHA-256 differs from update state: nothing written".into());
    }
    let requested = requested(state_path)?;
    if requested == Mode::Hs {
        state.sd_failed = false;
    }
    let mode = if state.sd_failed { Mode::Hs } else { requested };
    let next = image(&old, mode)?;
    let mut report = Report::default();
    report.say(format!("boot card mode: {:?}; takes effect on the next boot", mode));
    report.lines.extend(state.sd_fallback().map(str::to_string));
    if next == old {
        if execute {
            state.save(state_path).map_err(io("writing update state"))?;
        }
        report.unchanged = true;
        return Ok(report);
    }
    super::check_slot(card, &state)?;
    let previous = image(&old, Mode::Hs)?;
    if !execute {
        report.say("dry run: nothing written (--execute)");
        return Ok(report);
    }
    let entry = Entry { build: current.build.clone(), sha256: hash::sha256(&previous), len: previous.len() as u64 };
    super::state::write_slot(card, state_path, &previous, entry, &mut state)?;
    state.sd_trial = Some(mode);
    super::write_trial(card, state_path, &next, &current.build, &mut state, &mut report)?;
    Ok(report)
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
