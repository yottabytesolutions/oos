// SPDX-License-Identifier: GPL-2.0-or-later
//! Safe OS calls shared by system services and worker threads.

pub(crate) mod durable;
pub(crate) use durable::durable_replace;
mod cores;
pub use cores::{capacities, cpus, Cores};
mod ifaddrs;
pub use ifaddrs::ipv4_all;

#[cfg(any(target_os = "linux", test))]
mod shutdown;
#[cfg(target_os = "linux")]
pub use shutdown::{report_power_failure, DataShutdown};

use std::io;
use std::path::Path;

#[cfg(any(test, feature = "data-admission-test"))]
#[doc(hidden)]
pub mod admission_test;

/// Whether /proc/mounts lists the data filesystem at /oos.
pub fn oos_mounted(mounts: &str) -> bool {
    mounts.lines().any(|line| {
        let mut fields = line.split_whitespace().skip(1);
        fields.next() == Some("/oos") && fields.next() == Some("ext4")
    })
}

/// Reject data-area mutations during shutdown. Runtime files in /tmp remain writable.
pub fn check_data_write(path: &Path) -> io::Result<()> {
    #[cfg(any(test, feature = "data-admission-test"))]
    if let Some(closing) = admission_test::closing(path) {
        return data_write_admission(closing);
    }
    #[cfg(target_os = "linux")]
    if path.starts_with("/oos") {
        return data_write_admission(Path::new("/tmp/oos-data.closing").try_exists());
    }
    let _ = path;
    Ok(())
}

#[cfg(any(target_os = "linux", test, feature = "data-admission-test"))]
fn data_write_admission(closing: io::Result<bool>) -> io::Result<()> {
    if closing? {
        return Err(io::Error::new(io::ErrorKind::ReadOnlyFilesystem, "/oos is closed for shutdown"));
    }
    Ok(())
}

/// Signal one Linux process. Reject process-group, broadcast and init targets.
#[cfg(target_os = "linux")]
pub fn signal_process(pid: i32, signal: i32) -> io::Result<()> {
    if pid <= 1 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "expected a process ID above 1"));
    }
    // SAFETY: kill takes two scalar arguments. The positive PID selects one process.
    if unsafe { libc::kill(pid, signal) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Pins the calling thread and its future children to `cores`.
/// An empty set, or a platform other than Linux, leaves affinity unchanged.
#[cfg(target_os = "linux")]
pub fn pin_current_thread(cores: &[usize]) -> io::Result<()> {
    pin_thread(0, cores)
}

#[cfg(target_os = "linux")]
fn pin_thread(tid: libc::pid_t, cores: &[usize]) -> io::Result<()> {
    if cores.is_empty() {
        return Ok(());
    }
    if cores.iter().any(|&c| c >= libc::CPU_SETSIZE as usize) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "CPU index exceeds affinity set"));
    }
    // SAFETY: zero is a valid empty CPU set, every index fits it, and the kernel
    // reads the local set only during the call with its exact size.
    let rc = unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        for &c in cores {
            libc::CPU_SET(c, &mut set);
        }
        libc::sched_setaffinity(tid, std::mem::size_of::<libc::cpu_set_t>(), &set)
    };
    status(rc)
}

#[cfg(not(target_os = "linux"))]
pub fn pin_current_thread(_: &[usize]) -> io::Result<()> {
    Ok(())
}

/// Pin a Linux child before exec so the helper and its future threads inherit the CPU.
#[cfg(target_os = "linux")]
pub fn pin_child(command: &mut std::process::Command, cpu: usize) -> io::Result<()> {
    use std::os::unix::process::CommandExt;
    if cpu >= libc::CPU_SETSIZE as usize {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "CPU index exceeds affinity set"));
    }
    // SAFETY: only the affinity syscall runs in the child, using a stack set and a
    // validated CPU index. On failure last_os_error captures errno without allocating.
    unsafe { command.pre_exec(move || pin_current_thread(&[cpu])) };
    Ok(())
}

/// Pins the calling thread and gives it SCHED_FIFO priority 1. Restores its affinity if SCHED_FIFO fails.
#[cfg(target_os = "linux")]
pub fn fifo_current_thread(cpu: usize) -> io::Result<()> {
    let size = std::mem::size_of::<libc::cpu_set_t>();
    // SAFETY: zero is valid for both records, and libc uses the local pointers only during each call.
    let (mut original, mut param): (libc::cpu_set_t, libc::sched_param) = unsafe { std::mem::zeroed() };
    param.sched_priority = 1;
    // SAFETY: original is writable and size is exactly the size of its CPU set.
    status(unsafe { libc::sched_getaffinity(0, size, &mut original) })?;
    pin_current_thread(&[cpu])?;
    // SAFETY: pthread_self identifies the live calling thread, and param is initialized.
    let rc = unsafe { libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_FIFO, &param) };
    if rc == 0 {
        return Ok(());
    }
    let error = io::Error::from_raw_os_error(rc);
    // SAFETY: original remains live and contains the CPU set read from this same thread above.
    match status(unsafe { libc::sched_setaffinity(0, size, &original) }) {
        Ok(()) => Err(error),
        Err(restore) => Err(io::Error::other(format!("{error}, restoring affinity: {restore}"))),
    }
}

/// Gives the calling thread SCHED_IDLE priority (nice 10 if unavailable), then
/// pins it to `cores`. Priority failures are best effort; affinity errors return.
#[cfg(target_os = "linux")]
pub fn idle_current_thread(cores: &[usize]) -> io::Result<()> {
    // SAFETY: gettid has no pointer arguments. Zero initializes all sched_param
    // fields (including musl's extra fields); priority 0 is valid for SCHED_IDLE.
    // The scheduler reads the local parameter only during the syscall.
    let tid = unsafe {
        let tid = libc::syscall(libc::SYS_gettid) as libc::pid_t;
        let p: libc::sched_param = std::mem::zeroed();
        // musl's sched_setscheduler wrapper is a stub, so keep the direct syscall.
        if libc::syscall(libc::SYS_sched_setscheduler, tid, libc::SCHED_IDLE, &p as *const libc::sched_param) != 0 {
            libc::setpriority(libc::PRIO_PROCESS, tid as libc::id_t, 10);
        }
        tid
    };
    pin_thread(tid, cores)
}

#[cfg(not(target_os = "linux"))]
pub fn idle_current_thread(_: &[usize]) -> io::Result<()> {
    Ok(())
}

/// Available and total bytes of the filesystem holding `path`.
#[cfg(unix)]
pub fn fs_stats(path: impl AsRef<Path>) -> io::Result<(u64, u64)> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_ref().as_os_str().as_bytes())?;
    // SAFETY: statvfs is a C output record for which zero is valid. The path is
    // NUL terminated and both local values remain live throughout the call.
    let s = unsafe {
        let mut s: libc::statvfs = std::mem::zeroed();
        status(libc::statvfs(path.as_ptr(), &mut s))?;
        s
    };
    let fr = s.f_frsize as u64;
    Ok((s.f_bavail as u64 * fr, s.f_blocks as u64 * fr))
}

#[cfg(not(unix))]
pub fn fs_stats(_: impl AsRef<Path>) -> io::Result<(u64, u64)> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "filesystem statistics require Unix"))
}

/// Sets CLOCK_REALTIME to Unix seconds, preserving fractional nanoseconds.
#[cfg(target_os = "linux")]
pub fn set_realtime_clock(unix: f64) -> io::Result<()> {
    let ts = realtime_spec(unix);
    // SAFETY: ts is an initialized timespec that stays live throughout the call.
    // The kernel validates its values and CLOCK_REALTIME accepts this record.
    status(unsafe { libc::clock_settime(libc::CLOCK_REALTIME, &ts) })
}

#[cfg(any(target_os = "linux", test))]
fn realtime_spec(unix: f64) -> libc::timespec {
    libc::timespec { tv_sec: unix.floor() as _, tv_nsec: ((unix - unix.floor()) * 1e9) as _ }
}

#[cfg(not(target_os = "linux"))]
pub fn set_realtime_clock(_: f64) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "the clock is only set on the unit"))
}

#[cfg(any(unix, test))]
fn status(rc: libc::c_int) -> io::Result<()> {
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests;
