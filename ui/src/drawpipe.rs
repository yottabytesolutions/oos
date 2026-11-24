// SPDX-License-Identifier: GPL-2.0-or-later

//! Whole-batch placement and owned column differences. The UI takes one deposit per step.

mod accepted;
pub(crate) mod admission;
mod batch;
pub(crate) mod budget;
pub(crate) mod clock;
mod decoder;
mod feed;
mod frame;
pub(crate) mod funding;
pub mod host;
mod losses;
pub(crate) mod metrics;
mod pipe;
pub(crate) mod reuse;
pub(crate) mod route;
pub mod shutdown;
mod source;
pub(crate) mod stage;

pub(crate) use crate::picture::deposit::{TapRead, TapReservation};
pub use accepted::Accepted;
use batch::{Batch, Lease, Message};
pub use batch::{CodeLayout, CodesInto, DecodeInto, RawBatch};
#[cfg(any(target_os = "linux", test))]
pub use decoder::IndexedDecodeInto;
pub(crate) use feed::{same_pair, PreparedView};
#[cfg(test)]
pub use frame::draw_frame;
pub(crate) use frame::draw_frame_reusing;
pub use frame::Stats;
pub(crate) use losses::Lost;
pub use losses::{counters, Counters};
pub use pipe::{pipe, start_pipe, Pipe};
use pipe::{View, ViewState};
pub(crate) use source::{source_event, wake_source, SourceWake};

const WORKERS: usize = 2;

#[cfg(test)]
mod budget_tests;
#[cfg(test)]
mod counter_tests;
#[cfg(test)]
mod identity_tests;
#[cfg(test)]
pub(crate) mod test_scope;
#[cfg(test)]
mod tests;
