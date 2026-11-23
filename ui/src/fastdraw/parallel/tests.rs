// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// The poller's CPU from `/proc`: the busiest `cmpl_status_th*` thread, the others and other names left out.
#[test]
fn dma_poller_is_the_busiest_completion_thread() {
    let line = |name: &str, ut: u64, st: u64, cpu: usize| {
        format!("123 ({}) S 2 0 0 0 -1 2129984 0 0 0 0 {} {} 0 0 20 0 1 0 50 0 0 18446744073709551615 0 0 0 0 0 0 0 2147483647 0 0 0 0 17 {} 0 0 0 0 0", name, ut, st, cpu)
    };
    assert_eq!(stat_time_cpu(&line("cmpl status th", 7, 5, 2)), Some((12, 2)));
    let dir = std::env::temp_dir().join(format!("oos-poller-{}", std::process::id()));
    for (pid, name, ut, cpu) in [
        ("10", "cmpl_status_th0", 3, 0),
        ("11", "cmpl_status_th1", 9000, 1),
        ("12", "cmpl_status_th2", 0, 2),
        ("13", "busyloop", 99999, 3),
        ("self", "cmpl_status_th9", 99999, 3),
    ] {
        let d = dir.join(pid);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("comm"), format!("{}\n", name)).unwrap();
        std::fs::write(d.join("stat"), line(name, ut, 0, cpu)).unwrap();
    }
    let procs = || std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path());
    assert_eq!(dma_poller_cpu(procs()), Some(1));
    // a driver that never polled (interrupt mode): no core to avoid
    std::fs::write(dir.join("11/stat"), line("cmpl_status_th1", 0, 0, 1)).unwrap();
    std::fs::write(dir.join("10/stat"), line("cmpl_status_th0", 0, 0, 0)).unwrap();
    assert_eq!(dma_poller_cpu(procs()), None);
    let _ = std::fs::remove_dir_all(&dir);
}
