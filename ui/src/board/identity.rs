// SPDX-License-Identifier: GPL-2.0-or-later

use super::Frame;
use oos_core::change::Identity;

/// Source capture sequence and requested time per division and delay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StopView {
    pub(crate) capture: u64,
    pub(crate) scale: f64,
    pub(crate) delay: f64,
}
pub(crate) type StopReply = (StopView, Frame);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReplayLoss {
    Calibration,
    Successor,
}

#[derive(Default)]
pub(crate) struct StopReplies {
    pub(crate) frame: Option<StopReply>,
    calibration: Option<u64>,
    successor: Option<u64>,
}

impl StopReplies {
    #[cfg(any(target_os = "linux", test))]
    pub(in crate::board) fn calibrated_through(&mut self, seq: u64) {
        self.calibration = Some(seq);
        self.frame = None;
    }

    #[cfg(any(target_os = "linux", test))]
    pub(in crate::board) fn lost_through(&mut self, seq: u64) {
        self.successor = Some(seq);
        self.frame = None;
    }

    pub(crate) fn loss_for(&self, seq: u64) -> Option<ReplayLoss> {
        if self.calibration.is_some_and(|calibration| seq <= calibration) {
            Some(ReplayLoss::Calibration)
        } else {
            self.successor.is_some_and(|successor| seq <= successor).then_some(ReplayLoss::Successor)
        }
    }

    #[cfg(test)]
    pub(crate) fn inject_loss(&mut self, seq: u64, loss: ReplayLoss) {
        match loss {
            ReplayLoss::Calibration => self.calibrated_through(seq),
            ReplayLoss::Successor => self.lost_through(seq),
        }
    }
}

/// A value published with the settings and generator under Shared's lock.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Publication {
    identity: Identity,
    route: bool,
    start_epoch: u64,
}
impl Publication {
    pub(crate) fn new(identity: Identity, route: bool) -> Self {
        Self { identity, route, start_epoch: 0 }
    }
    /// Only an accepted RUN or SINGLE starts a new diagnostic lifetime.
    pub(crate) fn replace(&mut self, mut next: Self, started: bool) {
        next.start_epoch = if started { next.identity.epoch() } else { self.start_epoch };
        *self = next;
    }
    pub(crate) fn identity(self) -> Identity {
        self.identity
    }
    pub(crate) fn route(self) -> bool {
        self.route
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CaptureStatus {
    SingleHandoffMissed,
    StopUnconfirmed,
}

impl CaptureStatus {
    pub(super) fn message(self) -> &'static str {
        match self {
            Self::SingleHandoffMissed => "SINGLE completed but the UI did not take over",
            Self::StopUnconfirmed => "stop not confirmed by the hardware",
        }
    }
}

#[derive(Default)]
pub(super) struct SourceStatus {
    latest: String,
    diagnostic: Option<(u64, CaptureStatus)>,
}

impl SourceStatus {
    pub(super) fn set(&mut self, message: String) {
        self.latest = message;
    }

    pub(super) fn report(&mut self, publication: Publication, status: CaptureStatus, detail: Option<&str>) -> String {
        let message = status.message();
        let message = detail.map_or_else(|| message.to_owned(), |detail| format!("{message}: {detail}"));
        self.latest.clone_from(&message);
        self.diagnostic = Some((publication.start_epoch, status));
        message
    }

    pub(super) fn diagnostic(&self, publication: Publication) -> Option<CaptureStatus> {
        self.diagnostic.filter(|(epoch, _)| *epoch == publication.start_epoch).map(|(_, status)| status)
    }

    #[cfg(test)]
    pub(super) fn latest(&self) -> &str {
        &self.latest
    }

    #[cfg(test)]
    pub(super) fn take_latest(&mut self) -> String {
        std::mem::take(&mut self.latest)
    }
}

#[cfg(test)]
impl super::Shared {
    pub(crate) fn inject_stop_reply(&mut self, frame: Frame) {
        let view = self.stop_view.expect("a stopped view has been requested");
        self.stop_replies.frame = Some((view, frame));
    }
}
