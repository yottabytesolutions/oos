// SPDX-License-Identifier: GPL-2.0-or-later

//! STOP rereads retain the identity of the batch whose samples remain in the ring.

use crate::{board::Frame, ui::la::LaRec};
use oos_core::change::Identity;
use oos_hal::dho::{hz, seq::Settings};

/// The newest batch left in the ring, for the STOP rereads of each new view (`Shared::stop_view`), with the identity
/// its samples were captured under.
pub(super) struct Batch {
    pub(super) settings: Settings,
    pub(super) params: hz::Params,
    /// the frame of the batch read again
    pub(super) index: u32,
    raw: bool,
    identity: Identity,
    la: Option<LaRec>,
}

impl Batch {
    pub(super) fn new(
        settings: Settings,
        params: hz::Params,
        (index, raw): (u32, bool),
        identity: Identity,
        la: Option<LaRec>,
    ) -> Self {
        Self { settings, params, index, raw, identity, la }
    }

    pub(super) fn plan(&self, deep: Option<crate::deepmeas::Want>) -> Option<Settings> {
        super::hw::final_replay(&self.settings, &self.params, 1, self.raw, false, deep)
    }

    pub(super) fn tag(&self, mut frame: Frame) -> Frame {
        frame.la = self.la.clone();
        super::publication::tag(frame, self.identity)
    }

    /// A reread for a view: the trigger stays where the capture had it, and the frame keeps the capture's identity
    /// whatever was published since (a view change or CLEAR in STOP does not relabel old samples).
    pub(super) fn view(&self, frame: Frame, ti: f64) -> Frame {
        let mut frame = self.tag(frame);
        frame.fixed_ti = Some(ti);
        if let Some(la) = frame.la.as_mut() {
            // The digital read is relative to the hardware trigger. Match the origin of the analog window,
            // whose decoded pre can be clamped or rounded by the readout.
            la.t0 += (ti - frame.pre as f64) * frame.dt;
        }
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{identity::Publication, HwShared};
    use oos_core::change::{Request, Shadow, UiAcq};
    use oos_hal::sync::lock;

    #[test]
    fn clear_with_the_final_batch_rejects_its_stop_replay() {
        let model = crate::model::Model::default();
        let config = crate::model::config();
        let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
        let captured = shadow.identity();
        let mut settings = super::super::hw::settings(&model, false, &Default::default());
        settings.compress = 4;
        let params = hz::prep(settings.mask, oos_hal::dho::seq::tdiv_fs(settings.tdiv), 100_000, 0);
        let last_read = Batch::new(settings, params, (2, false), captured, None);
        let shared = HwShared::default();
        lock(&shared).acquisition = Publication::new(captured, false);
        assert!(last_read.plan(None).is_some(), "live pairs remain eligible for STOP reread");
        let before = last_read.tag(Frame {
            seq: 3,
            cfg: captured.gen(),
            stamp: captured.epoch(),
            ch: [vec![17.0], vec![], vec![], vec![]],
            ..Default::default()
        });
        assert!(super::super::publication::current(&lock(&shared), &before));
        let current = shadow.observe(&model, UiAcq::default(), &[Request::Clear], &config).unwrap().identity();
        lock(&shared).acquisition = Publication::new(current, false);
        let replay = last_read.tag(Frame { seq: 4, cfg: current.gen(), stamp: current.epoch(), ..before });
        assert_eq!((replay.cfg, replay.stamp), (captured.gen(), captured.epoch()));
        assert_ne!(current.epoch(), captured.epoch());
        assert!(last_read.plan(None).is_some(), "CLEAR preserves physical reread eligibility");
        super::super::publication::push_frames(&shared, vec![replay], oos_hal::dho::acq::Mode::Normal);
        assert!(lock(&shared).queue.is_empty(), "old samples cannot become a new-epoch record");
        let fresh = Batch::new(last_read.settings.clone(), params, (2, false), current, None);
        super::super::publication::push_frames(
            &shared,
            vec![fresh.tag(Frame { seq: 5, ..Default::default() })],
            oos_hal::dho::acq::Mode::Normal,
        );
        assert_eq!(lock(&shared).queue.back().unwrap().seq, 5, "current replay remains publishable");
    }

    #[test]
    fn a_stop_view_keeps_the_captures_logic_words_at_the_hardware_trigger() {
        use std::sync::Arc;
        let model = crate::model::Model::default();
        let words = Arc::new(vec![0, 1, 1, 0]);
        let la = LaRec { words: words.clone(), t0: -8e-9, dt: 4e-9 };
        let settings = super::super::hw::settings(&model, false, &Default::default());
        let params = hz::prep(settings.mask, oos_hal::dho::seq::tdiv_fs(settings.tdiv), 100_000, 0);
        let batch = Batch::new(settings, params, (0, false), Identity::default(), Some(la));
        // The view is to the right of the trigger. The decoder clamps pre to zero.
        let mut frame = batch.view(Frame { dt: 2e-9, ..Default::default() }, -6.0);
        // Include a fractional software-trigger offset, as the UI does for a stopped reread.
        frame.fixed_ti = Some(-5.5);
        let record = crate::board::to_record(&Arc::new(frame), &model, 64, 20e-9, 12e-9);
        let reread = record.la.unwrap();
        assert!(Arc::ptr_eq(&reread.words, &words));
        assert!((reread.t0 - -9e-9).abs() < 1e-21);
        assert_eq!(reread.dt, 4e-9);
    }

    #[test]
    fn a_stop_view_reread_keeps_the_identity_of_the_captured_batch() {
        let model = crate::model::Model::default();
        let config = crate::model::config();
        let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
        let captured = shadow.identity();
        let settings = super::super::hw::settings(&model, false, &Default::default());
        let params = hz::prep(settings.mask, oos_hal::dho::seq::tdiv_fs(settings.tdiv), 100_000, 0);
        let batch = Batch::new(settings, params, (2, false), captured, None);
        let current = shadow.observe(&model, UiAcq::default(), &[Request::Clear], &config).unwrap().identity();
        assert_ne!((current.gen(), current.epoch()), (captured.gen(), captured.epoch()));
        // the thread reads the view after the new publication: the decoded frame carries no identity of its own
        let reread =
            batch.view(Frame { seq: 9, cfg: current.gen(), stamp: current.epoch(), ..Default::default() }, 12.5);
        assert_eq!(
            (reread.cfg, reread.stamp),
            (captured.gen(), captured.epoch()),
            "a view must not relabel old samples"
        );
        assert_eq!(reread.fixed_ti, Some(12.5));
        assert_eq!(batch.index, 2);
    }
}
