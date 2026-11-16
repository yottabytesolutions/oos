// SPDX-License-Identifier: GPL-2.0-or-later

//! STOP rereads retain the identity of the batch whose samples remain in the ring.

use crate::board::Frame;
use oos_core::change::Identity;
use oos_hal::dho::{hz, seq::Settings};

pub(super) struct Read {
    settings: Settings,
    params: hz::Params,
    count: u32,
    raw: bool,
    identity: Identity,
}

impl Read {
    pub(super) fn new(settings: Settings, params: hz::Params, count: u32, raw: bool, identity: Identity) -> Self {
        Self { settings, params, count, raw, identity }
    }

    pub(super) fn plan(&self, ahead: bool, deep: Option<crate::deepmeas::Want>) -> Option<(Settings, hz::Params, u32)> {
        super::hw::final_replay(&self.settings, &self.params, self.count, self.raw, ahead, deep)
            .map(|settings| (settings, self.params, self.count))
    }

    pub(super) fn tag(&self, frame: Frame) -> Frame {
        super::publication::tag(frame, self.identity)
    }
}

/// The newest batch left in the ring, for the STOP rereads of each new view (`Shared::stop_view`), with the identity
/// its samples were captured under.
pub(super) struct Batch {
    pub(super) settings: Settings,
    pub(super) params: hz::Params,
    /// the frame of the batch read again
    pub(super) index: u32,
    identity: Identity,
}

impl Batch {
    pub(super) fn new(settings: Settings, params: hz::Params, index: u32, identity: Identity) -> Self {
        Self { settings, params, index, identity }
    }

    /// A reread for a view: the trigger stays where the capture had it, and the frame keeps the capture's identity
    /// whatever was published since (a view change or CLEAR in STOP does not relabel old samples).
    pub(super) fn view(&self, frame: Frame, ti: f64) -> Frame {
        super::publication::tag(Frame { fixed_ti: Some(ti), ..frame }, self.identity)
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
        let mut fresh = Some(Read::new(settings, params, 3, false, captured));
        let last_read = fresh.take().unwrap();
        let shared = HwShared::default();
        lock(&shared).acquisition = Publication::new(captured, false);
        assert!(last_read.plan(false, None).is_some(), "live pairs remain eligible for STOP reread");
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
        assert!(last_read.plan(false, None).is_some(), "CLEAR preserves physical reread eligibility");
        super::super::publication::push_frames(&shared, vec![replay]);
        assert!(lock(&shared).queue.is_empty(), "old samples cannot become a new-epoch record");
        let fresh = Read::new(last_read.settings.clone(), params, 3, false, current);
        super::super::publication::push_frames(&shared, vec![fresh.tag(Frame { seq: 5, ..Default::default() })]);
        assert_eq!(lock(&shared).queue.back().unwrap().seq, 5, "current replay remains publishable");
    }

    #[test]
    fn a_stop_view_reread_keeps_the_identity_of_the_captured_batch() {
        let model = crate::model::Model::default();
        let config = crate::model::config();
        let mut shadow = Shadow::new(&model, UiAcq::default(), &config).unwrap();
        let captured = shadow.identity();
        let settings = super::super::hw::settings(&model, false, &Default::default());
        let params = hz::prep(settings.mask, oos_hal::dho::seq::tdiv_fs(settings.tdiv), 100_000, 0);
        let batch = Batch::new(settings, params, 2, captured);
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
