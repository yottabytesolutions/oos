// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Hardware source: acquisitions through `oos_hal::dho::seq` (configure on change, arm, wait, read the whole record),
//! decoded into volts per channel. The UI aligns each frame with the software trigger engine (all 12 trigger types on
//! real data; the hardware itself triggers with an edge on the source at the level, in AUTO or NORMAL).

#[cfg(any(target_os = "linux", test))]
mod batch;
#[cfg(any(target_os = "linux", test))]
mod batch_timing;
#[cfg(any(target_os = "linux", test))]
mod dma_source;
pub(crate) mod header;
mod overlay;
#[cfg(target_os = "linux")]
mod raw;
#[cfg(any(target_os = "linux", test))]
mod segmented;
mod single;
#[cfg(target_os = "linux")]
mod startup;
#[cfg(any(target_os = "linux", test))]
mod stop;

#[cfg(target_os = "linux")]
mod acq;
mod comparator;
mod decode;
mod plan;
mod roll;
#[cfg(any(target_os = "linux", test))]
mod selfcal;
mod trigger;
#[cfg(any(target_os = "linux", test))]
mod trim;
#[cfg(any(target_os = "linux", test))]
mod units;
mod window;

#[cfg(target_os = "linux")]
use super::lifecycle;
#[cfg(any(target_os = "linux", test))]
use oos_hal::dho::seq::Settings;

#[cfg(target_os = "linux")]
pub use acq::spawn;
#[cfg(target_os = "linux")]
pub(super) use comparator::mask_map_init;
#[cfg(test)]
pub use comparator::mask_map_test_lock;
pub(super) use comparator::{mask_hw_from, mask_map_from, zone_hw_from};
pub use comparator::{mask_map, mask_map_set};
pub(super) use decode::decode_into_matched;
#[cfg(target_os = "linux")]
use decode::decode_into_with;
pub use decode::{codes_into, decode_into_probed};
#[cfg(test)]
pub use decode::{decode, decode_into};
#[cfg(any(target_os = "linux", test))]
pub use decode::{decode_probed, decode_with, la_record};
#[cfg(any(target_os = "linux", test))]
pub(super) use decode::{pair_step, probe_codes};
pub use plan::{avg_hw_ok, seg_max, settings};
#[cfg(any(target_os = "linux", test))]
pub use plan::{batch_plan, seg_read_plan};
#[cfg(test)]
pub use roll::roll_la_t0;
pub use roll::{slow_mode, ROLL_TDIV};
pub use trigger::serial_debug;
pub(super) use trigger::serial_hw_from;
#[cfg(any(target_os = "linux", test))]
pub use window::stop_plan;
#[cfg(any(target_os = "linux", test))]
pub(super) use window::{final_replay, raw_replay_settings};

#[cfg(test)]
pub(crate) mod single_fake;
#[cfg(test)]
mod tests;

/// The `cal.core_offset` line; `tight` or not.
pub(super) fn core_offset_from(conf: &str) -> bool {
    crate::sysconf::conf_value(None, conf, "cal.core_offset").as_deref() == Some("tight")
}
/// The `dvm.hw` line of system.conf; off unless `on`.
pub(super) fn dvm_hw_from(conf: &str) -> bool {
    crate::sysconf::conf_value(None, conf, "dvm.hw").as_deref() == Some("on")
}
/// OOS_PREARM set, else the `acq.prearm` line of system.conf; off unless `on`.
pub(super) fn prearm_from(env_set: bool, conf: &str) -> bool {
    env_set || crate::sysconf::conf_value(None, conf, "acq.prearm").as_deref() == Some("on")
}

/// Whether frame 0 of a batch of several frames is left out (issue #5): on unless `acq.frame0 = keep` in the settings
/// file (`BoardSwitches::frame0_drop`).
pub(super) fn frame0_drop_from(conf: &str) -> bool {
    crate::sysconf::conf_value(None, conf, "acq.frame0").as_deref() != Some("keep")
}

pub(super) fn zone_batch_from(conf: &str) -> u32 {
    crate::sysconf::conf_value(None, conf, "zone.batch").and_then(|v| v.parse::<u32>().ok()).unwrap_or(1).clamp(1, 1000)
}
/// A returned batch suppresses further SINGLE arms only for its captured pair. A new pair
/// must replace an obsolete batch that the UI cannot complete with. Accepted completion
/// retains the UI's hardware hold until the next RUN/SINGLE, including across reconfiguration.
fn single_wants_more(b: &oos_hal::dho::seq::Settings, done: Option<(u64, u64)>, pair: (u64, u64)) -> bool {
    !b.single || done != Some(pair)
}
