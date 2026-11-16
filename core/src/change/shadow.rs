// SPDX-License-Identifier: GPL-2.0-or-later

use super::{classify, normalize, refresh, snapshot_bytes, ChangeSet, Error, Generations, Request, UiAcq};
use crate::{config::Config, model::Model};

/// Exactly one previous normalized model and one scratch, each bounded to 64 KiB including capacities.
/// Updating these snapshots never uses Model's derived clone_from.
pub struct Shadow {
    previous: Model,
    scratch: Model,
    needs: UiAcq,
    generations: Generations,
}
impl Shadow {
    pub fn new(model: &Model, needs: UiAcq, config: &Config) -> Result<Self, Error> {
        if snapshot_bytes(model) >= 64 * 1024 {
            return Err(Error::SnapshotTooLarge);
        }
        let mut previous = model.clone();
        normalize(&mut previous, config);
        Ok(Self { scratch: previous.clone(), previous, needs, generations: Generations::default() })
    }
    pub fn observe(
        &mut self,
        model: &Model,
        needs: UiAcq,
        requests: &[Request],
        config: &Config,
    ) -> Result<ChangeSet, Error> {
        self.observe_with(model, needs, requests, config, |_| {})
    }
    pub fn observe_with(
        &mut self,
        model: &Model,
        needs: UiAcq,
        requests: &[Request],
        config: &Config,
        mut emit: impl FnMut(super::FieldDiff),
    ) -> Result<ChangeSet, Error> {
        if snapshot_bytes(model) >= 64 * 1024 {
            return Err(Error::SnapshotTooLarge);
        }
        if super::snapshot::projected_bytes(&self.scratch, model) >= 64 * 1024 {
            return Err(Error::SnapshotTooLarge);
        }
        refresh(&mut self.scratch, model);
        normalize(&mut self.scratch, config);
        if snapshot_bytes(&self.scratch) >= 64 * 1024 {
            return Err(Error::SnapshotTooLarge);
        }
        super::fields(&self.previous, &self.scratch, &mut emit);
        // An external RUN can override queued completion before STOP becomes the accepted model.
        // Only its Single-to-Auto sweep companion belongs to that request, even when run values are equal.
        let boundary = requests.iter().any(|r| matches!(r, Request::Run | Request::Single));
        let mut classes = super::classify_boundary(&self.previous, &self.scratch, self.needs, needs, boundary);
        if super::snapshot::projected_bytes(&self.previous, &self.scratch) >= 64 * 1024 {
            return Err(Error::SnapshotTooLarge);
        }
        if requests.contains(&Request::Run) && self.scratch.segments > 0 {
            classes |= super::Classes::HISTORY;
        }
        let changes = self.generations.resolve(classes, requests)?;
        refresh(&mut self.previous, &self.scratch);
        self.needs = needs;
        Ok(changes)
    }
    /// Normalize live settings, using the existing scratch to report only classes of effective repairs.
    /// No shadow values are copied into the acting model. The temporary legacy oracle uses this evidence.
    pub fn normalize_live(&mut self, model: &mut Model, config: &Config) -> Result<(bool, super::Classes), Error> {
        if snapshot_bytes(model) >= 64 * 1024 || super::snapshot::projected_bytes(&self.scratch, model) >= 64 * 1024 {
            return Err(Error::SnapshotTooLarge);
        }
        refresh(&mut self.scratch, model);
        let changed = normalize(model, config);
        let classes = classify(&self.scratch, model, self.needs, self.needs);
        Ok((changed, classes))
    }
    /// Compare a value outbox with the corresponding legacy writes, never changing the acting model.
    pub fn proposals_agree(&mut self, proposals: &super::Outbox, legacy: &Model) -> bool {
        if super::snapshot::projected_bytes(&self.scratch, legacy) >= 64 * 1024 {
            return false;
        }
        proposals.apply(&mut self.scratch);
        let mut agree = true;
        super::fields(&self.scratch, legacy, |f| {
            if f.changed() && !matches!(f.name(), "status" | "hw_stamp" | "hw_avg_clear") {
                agree = false;
            }
        });
        agree
    }
    pub fn needs(&self) -> UiAcq {
        self.needs
    }
    pub fn accepted(&self) -> &Model {
        &self.previous
    }
    pub fn normalized(&self) -> &Model {
        &self.scratch
    }
    pub fn identity(&self) -> super::Identity {
        self.generations.identity()
    }
    pub fn bytes(&self) -> (usize, usize) {
        (snapshot_bytes(&self.previous), snapshot_bytes(&self.scratch))
    }
}
