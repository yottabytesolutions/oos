// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

/// Helper threads the fast path spreads a batch over, besides the frame loop itself: one per core of
/// `helper_cores`; elsewhere all cores but two, at most four.
pub fn helpers() -> usize {
    match helper_cores().len() {
        0 => std::thread::available_parallelism().map_or(1, |n| n.get()).saturating_sub(2).min(4),
        n => n.min(4),
    }
}

/// The cores of the helpers: on a big.LITTLE SoC the frame loop and the acquisition thread keep the fast cores
/// (RK3399: two A72), the helpers take the small ones (four A53), but not the one the DMA completion poller spins
/// on (`dma_poller_cpu`): it gives the core up only every 100 polls, so a helper there waits for it, the frame loop
/// waits for the helper, and the end of the transfer is seen later. Found at the first batch (frames have come in,
/// so the poller has run).
fn helper_cores() -> &'static [usize] {
    static C: std::sync::OnceLock<Vec<usize>> = std::sync::OnceLock::new();
    C.get_or_init(|| {
        let little = crate::placement::little_core_list();
        let busy = std::fs::read_dir("/proc").ok().and_then(|d| dma_poller_cpu(d.flatten().map(|e| e.path())));
        let rest: Vec<usize> = little.iter().copied().filter(|&c| Some(c) != busy).collect();
        if rest.is_empty() {
            little
        } else {
            rest
        }
    })
}

/// The CPU of the busiest DMA completion poller (the xdma driver's kernel threads `cmpl_status_th<N>`, one bound to
/// each CPU, only the one serving the card-to-host engine works) among the process directories `procs`.
pub fn dma_poller_cpu(procs: impl Iterator<Item = std::path::PathBuf>) -> Option<usize> {
    procs
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit())))
        .filter(|p| std::fs::read_to_string(p.join("comm")).is_ok_and(|c| c.starts_with("cmpl_status_th")))
        .filter_map(|p| std::fs::read_to_string(p.join("stat")).ok().and_then(|s| stat_time_cpu(&s)))
        .max_by_key(|&(t, _)| t)
        .filter(|&(t, _)| t > 0)
        .map(|(_, c)| c)
}

/// CPU time (utime + stime, ticks) and last CPU of a `/proc/<pid>/stat` line.
fn stat_time_cpu(s: &str) -> Option<(u64, usize)> {
    // the name in parentheses may hold spaces: the fields count from after its closing one (field 3, the state)
    let f: Vec<&str> = s.get(s.rfind(')')? + 1..)?.split_whitespace().collect();
    let num = |i: usize| f.get(i).and_then(|x| x.parse::<u64>().ok());
    Some((num(11)? + num(12)?, num(36)? as usize))
}

/// Runs `work` on the calling thread and on up to `n` helper threads on the small cores, all together; returns what
/// each gave, the calling thread's first.
fn together<R: Send>(n: usize, work: impl Fn() -> R + Sync) -> Vec<R> {
    if n == 0 {
        return vec![work()];
    }
    let work = &work;
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..n)
            .map(|_| {
                s.spawn(move || {
                    let _ = oos_hal::system::pin_current_thread(helper_cores());
                    work()
                })
            })
            .collect();
        let mut out = vec![work()];
        out.extend(hs.into_iter().map(|h| h.join().expect("fast path helper")));
        out
    })
}

/// `items.iter().map(f).collect()` on the frame loop's thread and the helpers, in order. The items are handed out one
/// by one (a small core takes about twice as long as a fast one).
pub fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let n = helpers().min(items.len().saturating_sub(1));
    if n == 0 {
        return items.iter().map(&f).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let parts = together(n, || {
        let mut got = Vec::new();
        loop {
            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if i >= items.len() {
                return got;
            }
            got.push((i, f(&items[i])));
        }
    });
    let mut all: Vec<(usize, R)> = parts.into_iter().flatten().collect();
    all.sort_unstable_by_key(|x| x.0);
    all.into_iter().map(|x| x.1).collect()
}

/// `f` for every item, each item to one thread (the frame loop's or a helper's), handed out one by one.
pub fn par_each<T: Send>(items: Vec<T>, f: impl Fn(T) + Sync) {
    let n = helpers().min(items.len().saturating_sub(1));
    if n == 0 {
        items.into_iter().for_each(f);
        return;
    }
    let q = std::sync::Mutex::new(items.into_iter());
    together(n, || loop {
        let Some(x) = oos_hal::sync::lock(&q).next() else { return };
        f(x);
    });
}

#[cfg(test)]
mod tests;
