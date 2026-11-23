// SPDX-License-Identifier: GPL-2.0-or-later

use super::{timing, Mode, Reason, Status};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CARD_DEVICE: &str = "/sys/class/block/mmcblk0/device";
const DEBUGFS: &str = "/sys/kernel/debug";

trait Probe {
    fn resolve(&self, path: &Path) -> io::Result<PathBuf>;
    fn has_property(&self, path: &Path) -> io::Result<bool>;
    fn read_text(&self, path: &Path) -> io::Result<String>;
    fn ensure_debugfs(&self) -> bool;
    fn kernel_log(&self) -> Option<String>;
}

/// Capture the negotiated mode and running device-tree ceiling of the boot SD card.
pub fn detect() -> Status {
    detect_with(&System)
}

pub(super) fn card_key() -> Option<[u8; 16]> {
    let cid = fs::read_to_string(Path::new(CARD_DEVICE).join("cid")).ok()?;
    super::history::card_key(&cid)
}

fn detect_with(probe: &impl Probe) -> Status {
    let Some(host_path) = probe.resolve(Path::new(CARD_DEVICE)).ok().and_then(|path| path.parent().map(Path::to_owned))
    else {
        return Status::from_probe(Err(Reason::BootHost), Ok(Mode::Unknown));
    };
    let Some(host) = host_path.file_name().and_then(|name| name.to_str()).filter(|name| is_host(name)) else {
        return Status::from_probe(Err(Reason::BootHost), Ok(Mode::Unknown));
    };
    Status::from_probe(observed(probe, host), ceiling(probe, &host_path))
}

fn is_host(name: &str) -> bool {
    name.strip_prefix("mmc").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn observed(probe: &impl Probe, host: &str) -> Result<Mode, Reason> {
    let failure = if probe.ensure_debugfs() {
        let ios = Path::new(DEBUGFS).join(host).join("ios");
        if let Ok(text) = probe.read_text(&ios) {
            return timing::parse_ios(&text);
        }
        Reason::IosUnavailable
    } else {
        Reason::DebugfsUnavailable
    };
    probe.kernel_log().as_deref().and_then(|text| timing::from_log(text, host)).ok_or(failure)
}

fn ceiling(probe: &impl Probe, host: &Path) -> Result<Mode, Reason> {
    // A host class directory sits under its owning controller: controller/mmc_host/mmcN.
    let controller = host.parent().filter(|path| path.ends_with("mmc_host")).and_then(Path::parent);
    let node = controller.and_then(|path| probe.resolve(&path.join("of_node")).ok()).ok_or(Reason::DeviceTree)?;
    for (property, mode) in [("sd-uhs-sdr104", Mode::Sdr104), ("sd-uhs-sdr50", Mode::Sdr50)] {
        if probe.has_property(&node.join(property)).map_err(|_| Reason::DeviceTree)? {
            return Ok(mode);
        }
    }
    Ok(Mode::Hs)
}

struct System;

impl Probe for System {
    fn resolve(&self, path: &Path) -> io::Result<PathBuf> {
        fs::canonicalize(path)
    }

    fn has_property(&self, path: &Path) -> io::Result<bool> {
        path.try_exists()
    }

    fn read_text(&self, path: &Path) -> io::Result<String> {
        fs::read_to_string(path)
    }

    fn ensure_debugfs(&self) -> bool {
        let mounted = || fs::read_to_string("/proc/mounts").is_ok_and(|text| debugfs_mounted(&text));
        mounted()
            || Command::new("mount")
                .args(["-t", "debugfs", "debugfs", DEBUGFS])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
            // The start-up watchdog can mount debugfs concurrently.
            || mounted()
    }

    fn kernel_log(&self) -> Option<String> {
        let log = Command::new("dmesg").stderr(Stdio::null()).output().ok()?;
        log.status.success().then(|| String::from_utf8_lossy(&log.stdout).into_owned())
    }
}

fn debugfs_mounted(text: &str) -> bool {
    text.lines().any(|line| {
        let mut fields = line.split_whitespace().skip(1);
        fields.next() == Some(DEBUGFS) && fields.next() == Some("debugfs")
    })
}

#[cfg(test)]
mod tests;
