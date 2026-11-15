// SPDX-License-Identifier: GPL-2.0-or-later
//! The helper stops before its local mapping is released.
use super::*;
impl Gpu {
    /// Targets from `n` on are the monitor's (2 unless set: the script's memory targets). The panel has three
    /// scan-out buffers: with the old fixed 2, a panel frame into its third buffer took the monitor's half of the
    /// arena, and the monitor's frame sent right after it overwrote its layers while the GPU still drew them (black
    /// flashes on the panel with HDMI and Graphics = GPU, mostly with a menu open).
    pub fn monitor_targets_from(&mut self, n: usize) {
        self.monitor_from = n;
    }

    pub fn shutdown(&mut self) -> bool {
        self.shutdown_until(std::time::Instant::now() + Duration::from_secs(1))
    }

    pub fn shutdown_until(&mut self, deadline: std::time::Instant) -> bool {
        if let Some(done) = self.stopped {
            return done;
        }
        let _ = self.sock.shutdown(std::net::Shutdown::Write);
        let done = process::stop(&mut self.child, deadline);
        self.stopped = Some(done);
        done
    }
}

pub(super) fn spawn(helper: &str) -> Result<(std::process::Child, UnixStream), String> {
    let (ours, theirs) = UnixStream::pair().map_err(|e| format!("socket: {}", e))?;
    let child = std::process::Command::new(helper)
        .stdin(std::process::Stdio::from(OwnedFd::from(theirs)))
        .stdout(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {}", helper, e))?;
    Ok((child, ours))
}
