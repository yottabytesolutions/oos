// SPDX-License-Identifier: GPL-2.0-or-later

use super::{check_image, hash::hex, io, lock, short, Card, Entry, RecoveryTracking, Region, State};
use std::path::Path;

/// Reconcile recovery tracking at ordinary start-up. Only the update state may be written.
pub fn reconcile_recovery(card: &mut Card, state_path: &Path) -> Result<Option<String>, String> {
    let _lock = lock(state_path)?;
    let mut state = State::load(state_path).map_err(io("reading the update state"))?;
    let rolling_back = state.event.starts_with("rolling back from ");
    if state.staged.is_some() || (state.trial.is_some() && !rolling_back) {
        return Ok(None);
    }
    let Some(current) = &state.current else { return Ok(None) };
    card.check_layout()?;
    match identify(card, &state, current, rolling_back)? {
        Recovery::Current(id) => {
            if rolling_back {
                state.event = format!("rollback interrupted before replacing build {}", short(&current.build));
            }
            if state.recovery.loader_id != Some(id) || rolling_back {
                state.recovery.loader_id = Some(id);
                state.save(state_path).map_err(io("writing the update state"))?;
            }
            Ok(None)
        }
        Recovery::Previous(previous, id) => {
            let reason = format!("completed rollback from {} to {}", short(&current.build), short(&previous.build));
            state.track_recovery(previous, id);
            state.trial = None;
            state.event.clone_from(&reason);
            state.save(state_path).map_err(io("writing the update state"))?;
            Ok(Some(reason))
        }
        // A partial rollback keeps its verified slot and can be repeated.
        Recovery::Changed(_) if rolling_back => Ok(None),
        Recovery::Changed(describe) => {
            let reason = format!("recovery changed since build {}: {describe}", short(&current.build));
            mark_untracked(state_path, &mut state, &reason)?;
            Ok(Some(reason))
        }
    }
}

enum Recovery {
    Current([u8; 20]),
    Previous(Entry, [u8; 20]),
    Changed(String),
}

fn identify(card: &mut Card, state: &State, current: &Entry, rolling_back: bool) -> Result<Recovery, String> {
    let Some(header) = card.header(Region::Recovery).map_err(io("reading recovery"))? else {
        return Ok(Recovery::Changed("no recognizable OOS image".into()));
    };
    let len = header.len as u64;
    let id = header.id;
    let loader_id = state.recovery.loader_id;
    // A rollback can replace bytes outside the loader's hash, so check the whole image in that case.
    if !rolling_back && current.len == len && loader_id == Some(id) {
        return Ok(Recovery::Current(id));
    }
    let previous = state.previous.as_ref().filter(|entry| entry.len == len);
    let match_current = current.len == len && (rolling_back || loader_id.is_none());
    if previous.is_none() && !match_current {
        return Ok(Recovery::Changed(format!("image {:?}, {len} bytes, loader id {}", header.name, hex(&id))));
    }
    let (image, sha) = card.read_image(Region::Recovery, header.len).map_err(io("reading recovery"))?;
    if match_current && sha == current.sha256 {
        return Ok(Recovery::Current(id));
    }
    if let Some(previous) = previous.filter(|entry| entry.sha256 == sha) {
        return Ok(Recovery::Previous(previous.clone(), id));
    }
    let kind = match check_image(&image) {
        Ok(_) => format!("image {:?}", header.name),
        Err(error) => format!("damaged image ({error})"),
    };
    Ok(Recovery::Changed(format!("{kind}, {len} bytes, SHA-256 {}", hex(&sha))))
}

/// Clear install and rollback tracking before switching firmware. Returns the configured boot mode.
pub fn leave(state_path: &Path) -> Result<bool, String> {
    let _lock = lock(state_path)?;
    let mut state = State::load(state_path).map_err(io("reading the update state"))?;
    super::install::check_can_start(&state)?;
    mark_untracked(state_path, &mut state, "tracking cleared by oos-update leave")?;
    Ok(state.persist)
}

fn mark_untracked(state_path: &Path, state: &mut State, reason: &str) -> Result<(), String> {
    state.current = None;
    state.previous = None;
    state.recovery = RecoveryTracking { untracked: Some(reason.into()), ..RecoveryTracking::default() };
    state.save(state_path).map_err(io("writing the update state"))
}
