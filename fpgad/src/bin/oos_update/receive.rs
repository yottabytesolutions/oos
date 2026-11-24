// SPDX-License-Identifier: GPL-2.0-or-later

//! Receive a kexec install in rcS, then let the ordinary updater install the image.

use crate::{die, notice, put, read_file, tell, Args, NOTICE, STDOUT_CLOSED};
use oos_hal::update::{self, kexec, Card, InstallOpt, State};
use std::path::Path;

use super::messages;

pub(crate) fn finish(a: &Args, path: &Path, source: kexec::Receive) -> bool {
    match source {
        kexec::Receive::Legacy(flags) => legacy(a, path, flags),
        kexec::Receive::Staged(flags) => staged(a, path, flags).unwrap_or_else(|e| {
            refused(path, &e);
            false
        }),
    }
}

fn staged(a: &Args, path: &Path, flags: kexec::Flags) -> Result<bool, String> {
    let build = std::fs::read_to_string("/etc/oos-build").map_err(|e| format!("reading the running build: {e}"))?;
    let keys = update::trusted_keys();
    let mut card = a.open_card(true)?;
    let package = kexec::staged_package(&mut card, Path::new(kexec::HEADER_FILE), &build, &keys)?;
    // The staged candidate already occupies the slot that an active trial would need for rollback.
    if State::load(path).map_err(|e| format!("reading the update state: {e}"))?.trial.is_some() {
        return Err("the image in recovery is still on trial".into());
    }
    if a.val("--model").is_some() {
        return Err("boot detects the running model from hardware".into());
    }
    let model = update::models::running()?;
    let opt = Args::kexec_opt(flags, &model);
    installing(&build);
    Ok(finish_install(&mut card, path, &package, &keys, &opt, build.trim(), true))
}

fn refused(path: &Path, error: &str) {
    let message = messages::not_installed(&format!("staged receive was refused ({error})"));
    staged_failure(path, &message);
}

fn staged_failure(path: &Path, message: &str) {
    if let Err(e) = kexec::record_staged_refusal(path, message) {
        tell(&format!("recording the staged update failure: {e}"));
    }
    if let Err(e) = std::fs::write(NOTICE, format!("{message}\n")) {
        tell(&format!("writing the staged update notice: {e}"));
    }
    tell(message);
}

fn legacy(a: &Args, path: &Path, flags: kexec::Flags) -> bool {
    let package = read_file(kexec::PACKAGE);
    let build = update::running_build().unwrap_or_default();
    installing(&build);
    // Where the previous kernel put the initramfs, for a damaged package.
    let at = |n: &str| {
        std::fs::read(format!("/proc/device-tree/chosen/linux,{}", n))
            .ok()
            .map(|b| b.iter().fold(0u64, |a, &x| a << 8 | x as u64))
    };
    if let (Some(a0), Some(a1)) = (at("initrd-start"), at("initrd-end")) {
        tell(&format!("initramfs at {:#x}..{:#x}", a0, a1));
    }
    // The running build verified the package; this build may trust other keys (a key change by update).
    let mut keys = update::trusted_keys();
    if let Some(k) = std::fs::read_to_string(kexec::KEY_FILE)
        .ok()
        .and_then(|t| update::hash::unhex(t.trim()))
        .and_then(|b| <[u8; 32]>::try_from(b).ok())
    {
        keys.push(k);
    }
    let mut card = a.open_card(true).unwrap_or_else(|e| die(&e));
    let model = a.running_model(false);
    let opt = Args::kexec_opt(flags, &model);
    finish_install(&mut card, path, &package, &keys, &opt, &build, false)
}

fn installing(build: &str) {
    let short = update::short(build);
    let _ = std::fs::write(
        "/dev/tty1",
        format!("\nUpdate {}: started by kexec, installing it. Do not power off.\n", short),
    );
    tell(&format!("build {} started by kexec: kernel, card and /oos work; installing it", short));
}

fn finish_install(
    card: &mut Card,
    path: &Path,
    package: &[u8],
    keys: &[[u8; 32]],
    opt: &InstallOpt,
    build: &str,
    staged: bool,
) -> bool {
    let short = update::short(build);
    let report = match update::install(card, path, package, keys, opt) {
        Ok(report) => report,
        Err(error) => {
            install_failed(card, path, build, &error, staged);
            return false;
        }
    };
    report.lines.iter().for_each(|line| out!("{}", line));
    if report.written {
        notice(&messages::installed(short));
    } else {
        let result = update::kexec_clear(path, &format!("started build {} by kexec: nothing to install", short));
        if staged {
            if let Err(error) = result {
                tell(&format!("clearing the staged update state: {error}"));
            }
        }
    }
    if !staged {
        let _ = std::fs::remove_file(kexec::PACKAGE);
    }
    report.written
}

fn install_failed(card: &mut Card, path: &Path, build: &str, error: &str, staged: bool) {
    // A write that began names this build on trial. Restore the previous image so the watcher cannot confirm
    // a damaged recovery. A refusal before the write leaves the card alone.
    let st = match State::load(path) {
        Ok(state) => state,
        Err(e) if staged => {
            staged_failure(
                path,
                &format!(
                    "Staged install failed ({error}). Reading the update state failed ({e}). No automatic restart."
                ),
            );
            return;
        }
        Err(_) => State::default(),
    };
    let began = st.trial.is_some() && st.current.as_ref().is_some_and(|c| c.build.trim() == build.trim());
    let back = if began && st.previous.is_some() {
        match update::rollback(card, path, "its install after the kexec start failed", true, None) {
            Ok(r) => messages::restore_result("previous image restored", &r),
            Err(e) => messages::restore_failed(&e),
        }
    } else {
        messages::no_restore(began).to_string()
    };
    let m = format!(
        "Update {} ran when started by kexec, but its install failed: {}; {}",
        update::short(build),
        error,
        back
    );
    if staged {
        staged_failure(path, &m);
    } else {
        let _ = update::kexec_clear(path, &m);
        notice(&m);
        tell(&format!("the package stays at {} until the next start", kexec::PACKAGE));
    }
}
