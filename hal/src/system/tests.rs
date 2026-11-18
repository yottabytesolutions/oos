// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
#[cfg(unix)]
fn readable_wait_tracks_queued_datagrams() {
    use std::os::fd::AsFd;
    use std::os::unix::net::UnixDatagram;

    let (sender, receiver) = UnixDatagram::pair().unwrap();
    receiver.set_nonblocking(true).unwrap();
    assert!(!wait_readable(receiver.as_fd(), 0));
    sender.send(&[1]).unwrap();
    assert!(wait_readable(receiver.as_fd(), -1));
    assert!(wait_readable(receiver.as_fd(), 0));
    assert_eq!(receiver.recv(&mut [0]).unwrap(), 1);
    assert!(!wait_readable(receiver.as_fd(), 0));
}

#[test]
fn core_sets_keep_capacity_order_and_mask_boundaries() {
    let cores = Cores::of(&[(5, 1024), (0, 485), (4, 1024), (3, 485), (2, 485), (1, 485)]);
    assert_eq!(cpus(cores.big), vec![4, 5]);
    assert_eq!(cpus(cores.little), vec![0, 1, 2, 3]);
    assert_eq!(cores.little_count(), Some(4));
    assert_eq!(Cores::of(&[]), Cores::default());
    assert_eq!(Cores::of(&[(0, 1024), (1, 1024)]), Cores::default());
    assert_eq!(Cores::default().little_count(), None);
    assert_eq!(cpus(0), Vec::<usize>::new());
    assert_eq!(cpus(1 << 63 | 1), vec![0, 63]);
    assert_eq!(Cores::of(&[(64, 1024), (63, 1024), (0, 485)]).big, 1 << 63);
}

#[test]
fn realtime_seconds_and_fraction_keep_existing_conversion() {
    for (unix, seconds, nanos) in [
        (0.0, 0, 0),
        (1_794_398_400.25, 1_794_398_400, 250_000_000),
        (42.0, 42, 0),
        (-0.25, -1, 750_000_000),
        (1.999_999_999_9, 1, 999_999_999),
    ] {
        let ts = realtime_spec(unix);
        assert_eq!((ts.tv_sec, ts.tv_nsec), (seconds, nanos));
    }
}

#[test]
fn empty_affinity_leaves_the_thread_alone() {
    assert!(pin_current_thread(&[]).is_ok());
}

#[test]
#[cfg(unix)]
fn filesystem_stats_report_available_bytes_and_errors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (available, total) = fs_stats(root).unwrap();
    assert!(total > 0);
    assert!(available <= total);
    assert_eq!(fs_stats(".\0").unwrap_err().kind(), io::ErrorKind::InvalidInput);
    assert!(fs_stats(root.join("Cargo.toml/child")).is_err());
}

#[test]
#[cfg(not(target_os = "linux"))]
fn unsupported_clock_keeps_the_existing_error() {
    let error = set_realtime_clock(0.0).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    assert_eq!(error.to_string(), "the clock is only set on the unit");
}

#[test]
#[cfg(target_os = "linux")]
fn invalid_cpu_is_rejected_before_building_the_set() {
    for cpu in [libc::CPU_SETSIZE as usize, usize::MAX] {
        assert_eq!(pin_current_thread(&[cpu]).unwrap_err().kind(), io::ErrorKind::InvalidInput);
    }
}

#[cfg(target_os = "linux")]
fn affinity() -> Vec<usize> {
    // SAFETY: zero is a valid CPU set; sched_getaffinity writes only the supplied
    // local buffer, and CPU_ISSET is bounded by CPU_SETSIZE.
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();
        assert_eq!(libc::sched_getaffinity(0, std::mem::size_of_val(&set), &mut set), 0);
        (0..libc::CPU_SETSIZE as usize).filter(|&c| libc::CPU_ISSET(c, &set)).collect()
    }
}

#[test]
#[cfg(target_os = "linux")]
fn affinity_pins_only_the_calling_thread_and_is_inherited() {
    std::thread::spawn(|| {
        let before = affinity();
        pin_current_thread(&[]).unwrap();
        assert_eq!(affinity(), before);
        let cpu = before[0];
        pin_current_thread(&[cpu]).unwrap();
        assert_eq!(affinity(), vec![cpu]);
        assert_eq!(std::thread::spawn(affinity).join().unwrap(), vec![cpu]);
    })
    .join()
    .unwrap();
}
