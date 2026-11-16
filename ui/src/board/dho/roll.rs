// SPDX-License-Identifier: GPL-2.0-or-later

//! Roll stream control and bounded snapshot publication; scan retains complete sweeps.

use crate::board::{Frame, Shared};
use crate::model::NCH;

/// An incomplete SINGLE scan must collect a whole new sweep after a pair boundary.
#[cfg(any(target_os = "linux", test))]
pub(super) fn restart_scan(
    mode: crate::board::Slow,
    halted: bool,
    rolling: &mut Option<oos_hal::dho::roll::Params>,
) -> bool {
    if !halted && matches!(mode, crate::board::Slow::Scan { single: true, .. }) {
        return rolling.take().is_some();
    }
    false
}

/// Discard retained samples at a pair boundary without touching the running hardware.
#[cfg(any(target_os = "linux", test))]
pub(super) fn boundary(
    applied: Option<(u64, u64)>,
    identity: oos_core::change::Identity,
    ring: &mut [std::collections::VecDeque<f32>; NCH],
    sweep: &mut [Vec<f32>; NCH],
    ring_la: &mut std::collections::VecDeque<u16>,
    sweep_la: &mut Vec<f32>,
    cursor: &mut usize,
) -> bool {
    if !super::publication::boundary(applied, identity) {
        return false;
    }
    ring.iter_mut().for_each(|r| r.clear());
    sweep.iter_mut().for_each(|r| r.clear());
    ring_la.clear();
    sweep_la.clear();
    *cursor = 0;
    true
}

/// Whether roll may poll the stream; a stopped or rearmed stream starts with an empty screen.
pub(crate) fn roll_run(
    g: &mut Shared,
    rolling: &mut Option<oos_hal::dho::roll::Params>,
    ring: &mut [std::collections::VecDeque<f32>; NCH],
    ring_la: &mut std::collections::VecDeque<u16>,
) -> bool {
    if g.hold || std::mem::take(&mut g.rearm) {
        *rolling = None;
        ring.iter_mut().for_each(|r| r.clear());
        ring_la.clear();
        g.queue.clear();
    }
    !g.hold
}

/// A roll picture into the UI queue (also called on ticks without new chunks).
pub(crate) fn publish_roll(g: &mut Shared, f: Frame) {
    // STOP may have arrived during the read; keep the stopped picture and count no publication.
    if g.hold {
        return;
    }
    // Roll pictures overlap: only the newest snapshot is needed, always within QUEUE_MAX.
    g.queue.clear();
    g.queue.push_back(f.clone());
    g.frame = f;
    crate::drawpipe::Counters::add(&crate::drawpipe::counters().acquired, 1);
}

/// Scan retains complete sweeps; roll replaces its overlapping snapshot.
pub(crate) fn publish(g: &mut Shared, f: Frame, scan: bool) {
    if scan {
        // A complete sweep must not be dropped by the next partial picture.
        if g.queue.back().is_none_or(|b| b.partial) {
            g.queue.clear();
        }
        g.queue.push_back(f.clone());
        g.frame = f;
    } else {
        publish_roll(g, f);
    }
}

/// Apply STOP/rearm before reading another roll or scan chunk, releasing Shared before bus IO.
#[cfg(any(target_os = "linux", test))]
pub(super) fn poll(
    mode: crate::board::Slow,
    out: &crate::board::HwShared,
    rolling: &mut Option<oos_hal::dho::roll::Params>,
    ring: &mut [std::collections::VecDeque<f32>; NCH],
    ring_la: &mut std::collections::VecDeque<u16>,
    sh: &mut oos_hal::dho::scu::Shadows,
    bar: &mut impl oos_hal::dho::bus::Bus,
) -> bool {
    if mode == crate::board::Slow::Off {
        return true;
    }
    let was_rolling = rolling.is_some();
    let run = roll_run(&mut oos_hal::sync::lock(out), rolling, ring, ring_la);
    if was_rolling && rolling.is_none() {
        bar.apply(&super::lifecycle::roll_stop(sh));
    }
    run
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{identity::Publication, HwShared};
    use oos_core::change::{Request, Shadow, UiAcq};
    use oos_hal::sync::lock;

    #[test]
    fn clear_and_run_discard_preboundary_roll_and_scan_samples() {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        for scan in [false, true] {
            for request in [Request::Clear, Request::Run] {
                check_boundary_samples(scan, request);
            }
        }
    }

    fn check_boundary_samples(scan: bool, request: Request) {
        let model = crate::model::Model::default();
        let config = crate::model::config();
        let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
        let old = shadow.identity();
        let mut ring = std::array::from_fn(|_| [17.0; 8].into());
        let mut sweep = std::array::from_fn(|_| vec![17.0; 8]);
        let mut ring_la = [17u16; 8].into();
        let mut sweep_la = vec![17.0; 8];
        let mut cursor = 7;
        let id = shadow.observe(&model, UiAcq::default(), &[request], &config).unwrap().identity();
        assert_eq!(id.gen(), old.gen());
        assert_ne!(id.epoch(), old.epoch());
        assert!(boundary(
            Some((old.gen(), old.epoch())),
            id,
            &mut ring,
            &mut sweep,
            &mut ring_la,
            &mut sweep_la,
            &mut cursor
        ));
        assert_eq!(cursor, 0);
        assert!(ring.iter().all(|v| v.is_empty()) && sweep.iter().all(|v| v.is_empty()));
        assert!(ring_la.is_empty() && sweep_la.is_empty());
        let shared = HwShared::default();
        lock(&shared).acquisition = Publication::new(id, false);
        // An empty tick and the first new analog/LA chunk must both publish no old samples.
        for chunks in [0, 1] {
            for ch in 0..NCH {
                ring[ch].extend(std::iter::repeat_n(23.0, chunks));
                sweep[ch].extend(std::iter::repeat_n(23.0, chunks));
            }
            ring_la.extend(std::iter::repeat_n(23, chunks));
            sweep_la.extend(std::iter::repeat_n(23.0, chunks));
            let ch = if scan { sweep.clone() } else { ring.clone().map(|r| r.into_iter().collect()) };
            let words =
                if scan { sweep_la.iter().map(|&v| v as u16).collect() } else { ring_la.iter().copied().collect() };
            check_snapshot(&shared, id, scan, ch, words, chunks);
        }
        assert!(!boundary(
            Some((id.gen(), id.epoch())),
            id,
            &mut ring,
            &mut sweep,
            &mut ring_la,
            &mut sweep_la,
            &mut cursor
        ));
        assert_eq!(ring[0].front(), Some(&23.0), "steady-state samples survive");
        assert_eq!(sweep_la, [23.0]);
    }

    fn check_snapshot(
        shared: &HwShared,
        id: oos_core::change::Identity,
        scan: bool,
        ch: [Vec<f32>; NCH],
        words: Vec<u16>,
        chunks: usize,
    ) {
        let f = Frame {
            seq: chunks as u64 + 1,
            cfg: id.gen(),
            stamp: id.epoch(),
            scan,
            roll: !scan,
            partial: true,
            ch,
            la: Some(crate::ui::la::LaRec { t0: 0.0, dt: 1.0, words: std::sync::Arc::new(words) }),
            ..Default::default()
        };
        let mut g = lock(shared);
        assert!(super::super::publication::current(&g, &f));
        publish(&mut g, f, scan);
        assert!(g.frame.ch.iter().flatten().all(|&v| v == 23.0));
        assert!(g.frame.la.as_ref().unwrap().words.iter().all(|&v| v == 23));
        assert_eq!(g.frame.ch[0].len(), chunks);
    }
}
