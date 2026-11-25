// SPDX-License-Identifier: GPL-2.0-or-later

use super::messages;
use crate::{die, need_data, notice, put, restart_for_trial, state_path, tell, Args, STDOUT_CLOSED};
use oos_hal::bootcard;
use oos_hal::update::{self, card, hash::hex, kexec, pkg, State};
use std::path::Path;

/// The device tree section of the image the loader started, for a later kexec start (`kexec::BOOTED_DT`): in an
/// ordinary start `recovery` (nothing has written it yet), in a kexec start the tree the previous start passed on.
fn keep_booted_tree(a: &Args, kexec_start: bool) {
    let saved = read_booted_tree(a, kexec_start)
        .and_then(|dt| std::fs::write(kexec::BOOTED_DT, dt).map_err(|e| format!("saving the booted tree: {e}")));
    if let Err(error) = saved {
        tell(&format!(
            "the device tree of this start is unknown ({error}): power-cycle before updating or use --new-dt"
        ));
    }
}

fn read_booted_tree(a: &Args, kexec_start: bool) -> Result<Vec<u8>, String> {
    if kexec_start {
        return std::fs::read(kexec::DT_FILE).map_err(|e| format!("reading the tree passed by kexec: {e}"));
    }
    let mut card = a.open_card(false)?;
    let lba = card::Region::Recovery.lba();
    let header = card.read(lba, 2048).map_err(|e| format!("reading the recovery header: {e}"))?;
    let (offset, len) = kexec::tree_at(&header).filter(|&(_, len)| len != 0).ok_or("recovery has no readable tree")?;
    let sector = card::SECTOR as usize;
    let first = offset / sector;
    let skip = offset % sector;
    let bytes = card
        .read(lba + first as u64, (skip + len).next_multiple_of(sector))
        .map_err(|e| format!("reading the tree from recovery: {e}"))?;
    Ok(bytes[skip..skip + len].to_vec())
}

pub(crate) fn status(a: &Args) {
    let mut c = a.open_card(false).unwrap_or_else(|e| die(&e));
    let p = state_path(a);
    let st = match State::load(&p) {
        Ok(s) => s,
        Err(e) => {
            // not "no trial": the values below are defaults then
            tell(&format!("state        UNREADABLE ({}): {}; is /oos mounted?", p.display(), e));
            State::default()
        }
    };
    match c.boot_magic() {
        Ok(m) => out!(
            "boot magic   {} ({})",
            String::from_utf8_lossy(&m),
            match &m {
                b"OOSK" => "next power-on: OOS",
                b"KRNL" => "next power-on: stock Android",
                _ => "unexpected",
            }
        ),
        Err(e) => tell(&format!("boot magic   unreadable: {e}")),
    }
    match c.check_layout() {
        Ok(()) => match c.data_has_journal() {
            Ok(enabled) => out!("data journal {}", if enabled { "enabled" } else { "disabled" }),
            Err(e) => tell(&format!("data journal unavailable ({e})")),
        },
        Err(e) => tell(&format!("layout       {e}")),
    }
    super::slot_status::show(&mut c, &st, a.flag("--quick"));
    if let Some(reason) = st.untracked_reason() {
        out!("tracking     untracked ({reason})");
    }
    match st.trial {
        Some(n) => out!("trial        on trial, {} start(s) without confirmation", n),
        None => out!("trial        none"),
    }
    out!("mode         {}", if st.persist { "persist" } else { "oneshot" });
    if let Some(reason) = st.sd_fallback() {
        out!("card mode    {reason}");
    }
    if !st.event.is_empty() {
        out!("last event   {}", st.event);
    }
    if let Some(b) = read_text("/etc/oos-build") {
        out!("running      {}", b.trim());
    }
    out!("trusted keys {}", update::trusted_keys().iter().map(|k| hex(&pkg::key_id(k))).collect::<Vec<_>>().join(", "));
}

pub(crate) fn leave(a: &Args) {
    need_data(a);
    let persist = update::leave(&state_path(a)).unwrap_or_else(|e| die(&e));
    out!("Update tracking cleared. Current mode: {}.", if persist { "persist" } else { "oneshot" });
    out!("Next power-on follows the existing boot selection: OOS if armed, stock Android otherwise.");
    out!("After the next healthy OOS start, persist arms a valid recovery image for another power-on.");
    out!("In oneshot, the power-on after that OOS start boots stock Android.");
}

pub(crate) fn mode(a: &Args) {
    need_data(a);
    let p = state_path(a);
    match a.rest.get(1).map(String::as_str) {
        // The choice counts from the next power-on, so update boot selection now.
        Some("persist") => {
            update::set_mode(&p, true).unwrap_or_else(|e| die(&e));
            if let Err(e) = update::arm_checked(&mut a.open_card(true).unwrap_or_else(|e| die(&e)), &p) {
                out!("not selected for the next power-on yet: {}", e);
            }
        }
        Some("oneshot") => {
            update::set_mode(&p, false).unwrap_or_else(|e| die(&e));
            a.open_card(true).unwrap_or_else(|e| die(&e)).disarm_oos().unwrap_or_else(|e| die(&e));
        }
        None => {}
        Some(m) => die(&format!("mode '{}': oneshot or persist", m)),
    }
    let st = State::load(&p).unwrap_or_else(|e| die(&e.to_string()));
    out!(
        "mode {}",
        if st.persist {
            "persist: OOS arms itself again after every confirmed start (an unconfirmed start -> stock next time)"
        } else {
            "oneshot: the power-on after an OOS start boots stock (oos-boot-oos-again to stay)"
        }
    );
}

pub(crate) fn boot(a: &Args) {
    let p = state_path(a);
    let Some(cmdline) = read_text("/proc/cmdline") else { return };
    let word = kexec::receive(&cmdline);
    let kx = word.filter(|source| matches!(source, kexec::Receive::Staged(_)) || exists(Path::new(kexec::PACKAGE)));
    keep_booted_tree(a, word.is_some());
    let observed = bootcard::read_state(Path::new(bootcard::STATE_PATH));
    // A cold SDR104 observation can create the first update state on a mounted data partition.
    let first_start = word.is_none()
        && p == Path::new(update::STATE_PATH)
        && observed.ceiling() == bootcard::Mode::Sdr104
        && super::card_check::data_mounted();
    if !first_start && !p.parent().is_some_and(exists) {
        if kx.is_some() {
            notice(&messages::not_installed("/oos is not there"));
        }
        return; // no update state yet
    }
    let running = read_text("/etc/oos-build");
    let running = running.as_deref().map(str::trim).filter(|build| !build.is_empty());
    if word.is_some() {
        if let Some(build) = running {
            if let Err(error) = update::sd::observe_handoff(&p, build, observed) {
                tell(&error);
            }
        }
    }
    match kx {
        // not the trial start of anything: a failed install, or the install, whose trial begins at the next start
        Some(source) => {
            if super::receive::finish(a, &p, source) {
                restart_for_trial(a, &p);
            }
            return;
        }
        None if word.is_some() => {
            let m = messages::not_installed("its package did not arrive");
            clear_missing_package(&p, &m, tell);
            notice(&m);
            return;
        }
        None => record_ordinary_start(a, &p),
    }
    match update::boot_started_with_card(&p, running, observed) {
        Ok(update::Boot::NoTrial) => {}
        Ok(update::Boot::Trial(n)) => {
            tell(&format!("build {} on trial (start {})", update::short(running.unwrap_or("?")), n))
        }
        Ok(update::Boot::Dropped(m)) => tell(&m),
        Ok(update::Boot::Rollback) => rollback_trial(a, &p),
        Err(e) => tell(&e),
    }
}

fn read_text(path: &str) -> Option<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) => {
            tell(&format!("reading {path}: {error}"));
            None
        }
    }
}

fn exists(path: &Path) -> bool {
    match path.try_exists() {
        Ok(found) => found,
        Err(error) => {
            tell(&format!("checking {}: {error}", path.display()));
            false
        }
    }
}

fn clear_missing_package(path: &Path, event: &str, log: impl FnOnce(&str)) {
    if let Err(error) = update::kexec_clear(path, event) {
        log(&format!("clearing the kexec mark for the missing package: {error}"));
    }
}

fn rollback_trial(a: &Args, path: &Path) {
    let message = "\nUpdate not confirmed: restoring the previous build. Do not power off.\n";
    if let Err(error) = std::fs::write("/dev/tty1", message) {
        tell(&format!("writing the rollback message to the console: {error}"));
    }
    let result = a.open_card(true).and_then(|mut card| {
        update::rollback(&mut card, path, "it started a second time without having been confirmed", true, None)
    });
    match result {
        Ok(report) => {
            report.lines.iter().for_each(|line| out!("{line}"));
            let context = match State::load(path) {
                Ok(state) => format!("Update {}", state.event),
                Err(error) => {
                    tell(&format!("reading the update state after rollback: {error}"));
                    "Update rollback completed".into()
                }
            };
            notice(&messages::restore_result(&context, &report));
        }
        Err(error) => notice(&format!("Update on trial did not confirm, {}", messages::restore_failed(&error))),
    }
}

fn record_ordinary_start(a: &Args, path: &Path) {
    // Observe recovery before clearing a pending stage or processing an open trial.
    match a.open_card(false).and_then(|mut card| update::reconcile_recovery(&mut card, path)) {
        Ok(Some(reason)) => tell(&format!("update tracking: {reason}")),
        Ok(None) => {}
        Err(e) => tell(&format!("checking update tracking: {e}")),
    }
    match update::kexec_missed(path, false) {
        Ok(Some(m)) => notice(&m),
        Ok(None) => {}
        Err(e) => tell(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_package_logs_failure_to_clear_the_pending_mark() {
        let dir = std::env::temp_dir().join(format!("oos-kexec-clear-test-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("state");
        let mut pending = State::default();
        pending.kexec = Some("candidate".into());
        pending.save(&path).unwrap();
        let blocked = path.with_extension("new");
        std::fs::create_dir(&blocked).unwrap();

        let mut logs = Vec::new();
        clear_missing_package(&path, "missing package", |line| logs.push(line.to_owned()));
        assert_eq!(State::load(&path).unwrap(), pending);
        assert_eq!(logs.len(), 1);
        assert!(logs[0].contains("clearing the kexec mark for the missing package: writing the update state"));

        std::fs::remove_dir(blocked).unwrap();
        clear_missing_package(&path, "missing package", |line| panic!("unexpected log: {line}"));
        assert_eq!(State::load(&path).unwrap().kexec, None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
