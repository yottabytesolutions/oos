// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Flags, KEY};
use crate::update::{card::Region, hash, io, lock, pkg, Card, State};
use std::io::Read;
use std::path::Path;

/// Signed package capability for a kexec start that reads its image from the rollback slot.
pub const STAGED_LINE: (&str, &str) = ("trial", "staged");
/// Signed package header carried by a staged kexec start.
pub const HEADER_FILE: &str = "/oos-kexec.hdr";

/// Which payload a kexec start should install.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Receive {
    Legacy(Flags),
    Staged(Flags),
}

/// The kernel's install word. Unknown tokens and malformed words select no install.
pub fn receive(cmdline: &str) -> Option<Receive> {
    let mut words = cmdline.split_whitespace().filter_map(|w| w.strip_prefix(KEY));
    let mut parts = words.next()?.split(',');
    if parts.next() != Some("install") || words.next().is_some() {
        return None;
    }
    let mut flags = Flags::default();
    let mut staged = false;
    for part in parts {
        let flag = match part {
            "downgrade" => &mut flags.downgrade,
            "force" => &mut flags.force,
            "any-model" => &mut flags.any_model,
            "staged" => &mut staged,
            _ => return None,
        };
        if std::mem::replace(flag, true) {
            return None;
        }
    }
    Some(if staged { Receive::Staged(flags) } else { Receive::Legacy(flags) })
}

/// Install options for either kind of kexec start.
pub fn flags(cmdline: &str) -> Option<Flags> {
    match receive(cmdline)? {
        Receive::Legacy(flags) | Receive::Staged(flags) => Some(flags),
    }
}

/// Record a staged receive refusal even if its kexec mark is missing.
pub fn record_staged_refusal(state_path: &Path, event: &str) -> Result<(), String> {
    clear(state_path, event, true)
}

/// The kexec start ended without installing. Forget the pending mark and record the event.
pub fn kexec_clear(state_path: &Path, event: &str) -> Result<(), String> {
    clear(state_path, event, false)
}

fn clear(state_path: &Path, event: &str, record_without_mark: bool) -> Result<(), String> {
    let _lock = lock(state_path)?;
    let mut state = State::load(state_path).map_err(io("reading the update state"))?;
    if state.kexec.take().is_some() {
        state.sd_trial = None;
    } else if !record_without_mark {
        return Ok(());
    }
    state.event = event.into();
    state.save(state_path).map_err(io("writing the update state"))
}

/// Verify the carried header and the slot image before handing a reconstructed package to install.
pub fn staged_package(
    card: &mut Card,
    header_path: &Path,
    running_build: &str,
    keys: &[[u8; 32]],
) -> Result<Vec<u8>, String> {
    let mut package = Vec::new();
    std::fs::File::open(header_path)
        .and_then(|file| file.take((pkg::MAX_HEADER + 1) as u64).read_to_end(&mut package))
        .map_err(|e| format!("reading the staged header: {e}"))?;
    let header = pkg::verify_header(&package, keys).map_err(|e| format!("staged header: {e}"))?;
    let manifest = header.manifest;
    if package.len() != header.data_offset {
        return Err("staged header contains trailing bytes".into());
    }
    if !manifest.extra.iter().any(|(key, value)| key == STAGED_LINE.0 && value == STAGED_LINE.1) {
        return Err("signed header has no 'trial staged' line".into());
    }
    let running_build = running_build.trim();
    if running_build.is_empty() || manifest.build != running_build {
        return Err(format!(
            "staged header build '{}' does not match running build '{}'",
            manifest.build, running_build
        ));
    }
    let size = usize::try_from(manifest.size)
        .ok()
        .filter(|&size| (1..=Region::Slot.max()).contains(&size))
        .ok_or_else(|| format!("staged image size {} does not fit the rollback slot", manifest.size))?;
    card.check_layout()?;
    package.resize(header.data_offset + size, 0);
    let image = &mut package[header.data_offset..];
    card.read_region_into(Region::Slot, image)
        .map_err(|e| format!("reading the staged image from the rollback slot: {e}"))?;
    if hash::sha256(image) != manifest.sha256 {
        return Err("staged image SHA-256 mismatch in the rollback slot".into());
    }
    Ok(package)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staged_and_legacy_words_select_only_their_own_payload() {
        let flags = Flags { downgrade: true, force: true, any_model: true };
        assert_eq!(receive("quiet oos.kexec=install,staged,downgrade,force,any-model"), Some(Receive::Staged(flags)));
        assert_eq!(receive("quiet oos.kexec=install,downgrade,force,any-model"), Some(Receive::Legacy(flags)));
        for cmdline in [
            "oos.kexec=staged",
            "oos.kexec=staged,install",
            "oos.kexec=install,unknown",
            "oos.kexec=install,",
            "oos.kexec=install,force,force",
            "oos.kexec=install,staged,staged",
            "oos.kexec=install oos.kexec=install,staged",
        ] {
            assert_eq!(receive(cmdline), None, "{cmdline}");
        }
    }
}
