// SPDX-License-Identifier: GPL-2.0-or-later

//! Where the time of a dense fast-path frame goes (fork, docs/fork/ac-trig.md): decoding to volts, the level of a
//! filtered trigger path (the frame mean), the near and the wide trigger search, and the whole frame. Sums since the
//! start in microseconds, for the status line; a measurement takes the difference of two readings.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// A part of a frame's work.
#[derive(Clone, Copy)]
pub enum Part {
    /// samples to volts: the whole frame (decode) or the near window of the trigger source (codes path)
    Decode,
    /// the software trigger level of AC or LF-reject coupling (the frame mean)
    Level,
    Near,
    /// the wide search, with its decode of the whole source on the codes path
    Wide,
    /// the whole frame, decode and deposit included
    Frame,
    /// picture worker wall time, including channel reduction completion
    Merge,
}

const PARTS: usize = 6;

static NS: [AtomicU64; PARTS] = [const { AtomicU64::new(0) }; PARTS];
static FRAMES: AtomicU64 = AtomicU64::new(0);
static WIDES: AtomicU64 = AtomicU64::new(0);

/// Adds the time since `t0` to `part`.
pub fn add(part: Part, t0: Instant) {
    add_at(part, t0, Instant::now());
}

fn add_at(part: Part, t0: Instant, now: Instant) {
    let ns = now.saturating_duration_since(t0).as_nanos().min(u64::MAX as u128) as u64;
    NS[part as usize].fetch_add(ns, Ordering::Relaxed);
    match part {
        Part::Frame => FRAMES.fetch_add(1, Ordering::Relaxed),
        Part::Wide => WIDES.fetch_add(1, Ordering::Relaxed),
        _ => 0,
    };
}

/// ` cost frames N wide N us decode/level/near/wide/frame a/b/c/d/e` (sums since the start).
pub fn summary() -> String {
    let us: Vec<String> =
        NS[..Part::Merge as usize].iter().map(|n| (n.load(Ordering::Relaxed) / 1000).to_string()).collect();
    format!(
        " cost frames {} wide {} us decode/level/near/wide/frame {}",
        FRAMES.load(Ordering::Relaxed),
        WIDES.load(Ordering::Relaxed),
        us.join("/")
    )
}

/// Cumulative picture-worker time, independent of the rolling quantile window.
pub(crate) fn merge_us() -> u64 {
    NS[Part::Merge as usize].load(Ordering::Relaxed) / 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    // Break it catches: a part added to another part's sum, or the frame and wide counts not following their parts.
    #[test]
    fn parts_add_to_their_own_sums() {
        let parse = |s: &str| -> (u64, u64, Vec<u64>) {
            let w: Vec<&str> = s.split_whitespace().collect();
            (w[2].parse().unwrap(), w[4].parse().unwrap(), w[7].split('/').map(|x| x.parse().unwrap()).collect())
        };
        let (f0, w0, a) = parse(&summary());
        let now = Instant::now();
        let t = now - std::time::Duration::from_millis(5);
        add_at(Part::Wide, t, now);
        add_at(Part::Frame, t, now);
        let merge_before = merge_us();
        add_at(Part::Merge, t, now);
        assert!(merge_us() >= merge_before + 5000);
        let (f1, w1, b) = parse(&summary());
        assert!(f1 > f0 && w1 > w0);
        assert!(b[3] >= a[3] + 5000 && b[4] >= a[4] + 5000);
    }
}
