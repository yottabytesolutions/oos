// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Bounded evidence for kexec_load, with the common watchdog handoff at the jump.

use super::{arguments, evidence, undo::report};
use std::path::Path;
use std::process::Command;

fn warn(result: std::io::Result<()>) {
    if let Err(e) = result {
        report(&format!("evidence failed: {e}; continuing"));
    }
}

pub(super) fn load(kernel: &[u8], initrd: &[u8], tree: &[u8], cmdline: &str, log: &str) -> Result<(), String> {
    super::budget::check(kernel.len(), initrd.len(), tree.len())?;
    let dir = Path::new("/tmp/oos-kexec");
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {}", dir.display(), e))?;
    let r = (|| {
        let from = super::segments_from(log);
        let saved = Path::new(evidence::DIR);
        warn(evidence::begin(saved, kernel.len(), initrd.len(), tree, from));
        for (n, d) in [("Image", kernel), ("initrd", initrd), ("tree.dtb", tree)] {
            std::fs::write(dir.join(n), d).map_err(|e| format!("{}: {}", n, e))?;
        }
        let (status, stdout, stderr) =
            evidence::run(Command::new(super::TOOL).args(arguments::legacy(dir, from, cmdline)))?;
        warn(evidence::write(saved, "stdout", &stdout, evidence::TEXT_LIMIT));
        warn(evidence::write(saved, "stderr", &stderr, evidence::TEXT_LIMIT));
        warn(evidence::write(saved, "load.log", status.to_string().as_bytes(), evidence::TEXT_LIMIT));
        if !status.success() {
            let err = String::from_utf8_lossy(&stderr);
            return Err(format!("kexec -l: {}", err.lines().last().unwrap_or("failed")));
        }
        Ok(())
    })();
    // The kernel has its own copy now; the bounded persistent copy remains across reset.
    let _ = std::fs::remove_dir_all(dir);
    r
}

fn timestamp(label: &str) -> String {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY: ts is a writable timespec; CLOCK_MONOTONIC requires no other arguments.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts) };
    let mono = if rc == 0 {
        format!("{}.{:09}", ts.tv_sec, ts.tv_nsec)
    } else {
        format!("unavailable ({})", std::io::Error::last_os_error())
    };
    let uptime = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
    format!("{label}: monotonic_s={mono} uptime_s={}", uptime.split_whitespace().next().unwrap_or("unknown"))
}

/// kexec-tools jump marker, prepared while persistent storage is writable.
#[derive(Debug)]
pub struct Jump;

/// Call while /oos is writable, after quiescing. Failure is diagnostic only, never a new update failure.
pub fn prepare_jump() -> Jump {
    let mut lines = String::new();
    let mut log = |s: &str| {
        report(s);
        lines.push_str(s);
        lines.push('\n');
    };
    log(&timestamp("before data shutdown"));
    let boot = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").unwrap_or_default();
    log(&format!("boot_id={}", boot.trim()));
    warn(evidence::write(Path::new(evidence::DIR), "jump.log", lines.as_bytes(), evidence::TEXT_LIMIT));
    Jump
}

impl Jump {
    /// Mark the jump before the final watchdog ping. Returns only on failure.
    pub fn start(self) -> String {
        super::watchdog::start(|| report(&timestamp("jump")))
    }
}
