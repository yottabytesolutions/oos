// SPDX-License-Identifier: GPL-2.0-or-later

mod outbox;
use super::Classes;
pub use outbox::{Command, Outbox};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Requester {
    Autoset,
    Bode,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    requester: Requester,
    run: u64,
    stage: u64,
}
impl Token {
    /// Callers use a new run id per invocation and a strictly increasing stage id, including retries.
    pub fn new(requester: Requester, run: u64, stage: u64) -> Self {
        Self { requester, run, stage }
    }
    pub fn run(self) -> u64 {
        self.run
    }
    pub fn stage(self) -> u64 {
        self.stage
    }
    pub fn requester(self) -> Requester {
        self.requester
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Clear,
    Run,
    Single,
    CreateMask,
    MaskReset,
    StatsReset,
    Replay,
    Epoch(Token),
}
impl Request {
    pub fn classes(self) -> Classes {
        use Classes as C;
        match self {
            Self::Clear => C::PHOSPHOR | C::STATS | C::MASK | C::ZONE | C::HW_AVERAGE | C::MATH | C::VIEW,
            Self::Run => C::HW_AVERAGE,
            Self::Single => C::PHOSPHOR | C::STATS | C::HW_AVERAGE,
            Self::CreateMask | Self::MaskReset => C::MASK,
            Self::StatsReset => C::STATS,
            Self::Replay => C::MAPPING,
            Self::Epoch(_) => C::NONE,
        }
    }
    fn epoch(self) -> bool {
        matches!(self, Self::Clear | Self::Run | Self::Single | Self::Epoch(_))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Full,
    PayloadTooLarge,
    SnapshotTooLarge,
    DuplicateToken,
    GenerationOverflow,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Identity {
    gen: u64,
    epoch: u64,
    stats_gen: u64,
}
impl Identity {
    pub fn gen(self) -> u64 {
        self.gen
    }
    pub fn epoch(self) -> u64 {
        self.epoch
    }
    pub fn stats_gen(self) -> u64 {
        self.stats_gen
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Generations {
    identity: Identity,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Resolution {
    token: Token,
    epoch: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeSet {
    classes: Classes,
    identity: Identity,
    epochs: [Option<Resolution>; 64],
}
impl ChangeSet {
    pub fn classes(&self) -> Classes {
        self.classes
    }
    pub fn identity(&self) -> Identity {
        self.identity
    }
    pub fn epoch(&self, token: Token) -> Option<u64> {
        self.epochs.iter().flatten().find(|r| r.token == token).map(|r| r.epoch)
    }
    pub fn resolutions(&self) -> impl Iterator<Item = (Token, u64)> + '_ {
        self.epochs.iter().flatten().map(|r| (r.token, r.epoch))
    }
    pub fn accepts(&self, token: Token, record_epoch: u64) -> bool {
        self.epoch(token) == Some(record_epoch)
    }
}
impl Generations {
    /// Validate first: overflow and duplicate requests leave identities unchanged and lose no token.
    pub(super) fn resolve(&mut self, mut classes: Classes, requests: &[Request]) -> Result<ChangeSet, Error> {
        if requests.len() > 64 {
            return Err(Error::Full);
        }
        for (i, r) in requests.iter().enumerate() {
            if matches!(r, Request::Epoch(_)) && requests[..i].contains(r) {
                return Err(Error::DuplicateToken);
            }
            classes |= r.classes();
        }
        let mut id = self.identity;
        let advance = |n: u64, yes: bool| n.checked_add(u64::from(yes)).ok_or(Error::GenerationOverflow);
        id.gen = advance(id.gen, classes.contains(Classes::ACQ))?;
        id.stats_gen = advance(id.stats_gen, classes.contains(Classes::STATS))?;
        id.epoch = advance(id.epoch, requests.iter().any(|r| r.epoch()))?;
        let mut epochs = [None; 64];
        for (i, r) in requests.iter().enumerate() {
            if let Request::Epoch(token) = r {
                epochs[i] = Some(Resolution { token: *token, epoch: id.epoch });
            }
        }
        self.identity = id;
        Ok(ChangeSet { classes, identity: id, epochs })
    }
    pub fn identity(&self) -> Identity {
        self.identity
    }
}
