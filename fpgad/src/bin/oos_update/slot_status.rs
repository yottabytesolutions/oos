// SPDX-License-Identifier: GPL-2.0-or-later

use crate::{die, put, STDOUT_CLOSED};
use oos_hal::update::{card, hash::hex, Card, Entry, State};

pub(crate) fn show(card: &mut Card, state: &State, quick: bool) {
    show_image(card, card::Region::Recovery, "recovery", state.current.as_ref(), quick);
    if let Some(message) = state.rollback_slot_status() {
        out!("rollback     {}", message);
    } else {
        show_image(card, card::Region::Slot, "rollback", state.previous.as_ref(), quick);
    }
}

fn show_image(card: &mut Card, region: card::Region, label: &str, entry: Option<&Entry>, quick: bool) {
    match card.header(region) {
        Ok(Some(h)) if quick => {
            let known = entry
                .filter(|e| e.len == h.len as u64)
                .map_or_else(|| "untracked".into(), |e| format!("same size as build {}", e.build));
            out!("{label:<12} '{}', {} bytes, {}, loader id {}", h.name, h.len, known, &hex(&h.id)[..16]);
        }
        Ok(Some(h)) => {
            let (bytes, sha) = card.read_image(region, h.len).unwrap_or_else(|e| die(&e.to_string()));
            let ok = card::check_image(&bytes).map_or_else(|e| format!("DAMAGED: {e}"), |_| "loader id ok".into());
            let known =
                entry.filter(|e| e.sha256 == sha).map_or_else(|| "untracked".into(), |e| format!("build {}", e.build));
            out!("{label:<12} '{}', {} bytes, {}, {}, sha256 {}", h.name, h.len, known, ok, &hex(&sha)[..16]);
        }
        Ok(None) => out!("{label:<12} no image"),
        Err(e) => out!("{label:<12} unreadable: {e}"),
    }
}
