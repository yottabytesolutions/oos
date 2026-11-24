// SPDX-License-Identifier: GPL-2.0-or-later
//! The helper stops before its local mapping is released.
use super::*;
impl Gpu {
    /// A scan-out buffer of the display as target `index` (the helper keeps the descriptor).
    pub fn target_dmabuf(&mut self, index: usize, w: i32, h: i32, pitch: i32, fd: OwnedFd) -> Result<(), String> {
        self.target(index, w, h, pitch, Some(fd))
    }

    /// A target in the helper's own memory, read back after every frame (tests).
    pub fn target_memory(&mut self, index: usize, w: i32, h: i32) -> Result<(), String> {
        self.target(index, w, h, w * 4, None)
    }

    fn target(&mut self, index: usize, w: i32, h: i32, pitch: i32, fd: Option<OwnedFd>) -> Result<(), String> {
        let target = self.targets.get_mut(index).ok_or("target index out of range")?;
        let memory = fd.is_none();
        let msg = TargetMsg {
            magic: MAGIC,
            kind: K_TARGET,
            index: index as u32,
            w,
            h,
            pitch,
            fourcc: FOURCC_XRGB8888,
            memory: u32::from(memory),
        };
        send_fd(&self.sock, bytes(&msg), fd.as_ref().map(AsRawFd::as_raw_fd)).map_err(|e| format!("target: {e}"))?;
        drop(fd);
        *target = None;
        self.wait(Duration::from_secs(2))?;
        self.targets[index] = Some((w, h, memory));
        Ok(())
    }

    /// Targets from `n` on are the monitor's (2 unless set: the script's memory targets). The panel has three
    /// scan-out buffers: with the old fixed 2, a panel frame into its third buffer took the monitor's half of the
    /// arena, and the monitor's frame sent right after it overwrote its layers while the GPU still drew them (black
    /// flashes on the panel with HDMI and Graphics = GPU, mostly with a menu open).
    pub fn monitor_targets_from(&mut self, n: usize) {
        self.monitor_from = n;
    }

    pub(crate) fn pinned_cpu(&self) -> Option<usize> {
        let status = std::fs::read_to_string(format!("/proc/{}/status", self.child.id())).ok()?;
        status.lines().find_map(|line| line.strip_prefix("Cpus_allowed_list:"))?.trim().parse().ok()
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

pub(super) fn spawn(helper: &str, cpu: Option<usize>) -> Result<(std::process::Child, UnixStream), String> {
    let (ours, theirs) = UnixStream::pair().map_err(|e| format!("socket: {}", e))?;
    let mut command = std::process::Command::new(helper);
    if let Some(cpu) = cpu {
        oos_hal::system::pin_child(&mut command, cpu).map_err(|e| format!("GPU CPU {cpu}: {e}"))?;
    }
    let child = command
        .stdin(std::process::Stdio::from(OwnedFd::from(theirs)))
        .stdout(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("{}: {}", helper, e))?;
    Ok((child, ours))
}
