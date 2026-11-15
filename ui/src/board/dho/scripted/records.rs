// SPDX-License-Identifier: GPL-2.0-or-later

//! Synthetic consumer snapshots. These never exercise the hardware producer loops.

use crate::board::scripted::BatchKind;
use crate::board::Frame;

pub(super) fn characterise(frame: &mut Frame, kind: BatchKind, tdiv: f64) {
    frame.hw_avg = kind == BatchKind::HardwareAverage;
    if !matches!(kind, BatchKind::Roll | BatchKind::Scan) {
        return;
    }
    frame.roll = kind == BatchKind::Roll;
    frame.scan = kind == BatchKind::Scan;
    let points = frame.ch.iter().map(Vec::len).max().unwrap_or(1).max(1);
    frame.dt = 10.0 * tdiv / points as f64;
    let sixths = (frame.seq as usize - 1) % 6 + 1;
    frame.partial = sixths < 6;
    let keep = points * sixths / 6;
    for channel in &mut frame.ch {
        channel.truncate(keep);
    }
    frame.pre = if frame.roll { keep.saturating_sub(points / 2 + 1) } else { points / 2 };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_flags_and_two_sweep_boundaries_are_explicit() {
        for kind in [BatchKind::Roll, BatchKind::Scan] {
            for seq in 1..=12 {
                let mut frame = Frame { seq, ch: std::array::from_fn(|_| vec![1.0; 120]), ..Default::default() };
                characterise(&mut frame, kind, 0.2);
                assert_eq!(frame.roll, kind == BatchKind::Roll);
                assert_eq!(frame.scan, kind == BatchKind::Scan);
                assert_eq!(frame.partial, seq % 6 != 0);
                assert_eq!(frame.ch[0].len(), ((seq as usize - 1) % 6 + 1) * 20);
                assert_eq!(frame.dt, 2.0 / 120.0);
                assert!(!frame.envelope);
            }
        }
        let mut frame = Frame::default();
        characterise(&mut frame, BatchKind::HardwareAverage, 0.0002);
        assert!(frame.hw_avg && !frame.roll && !frame.scan);
    }
}
