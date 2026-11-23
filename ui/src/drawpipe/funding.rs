// SPDX-License-Identifier: GPL-2.0-or-later
//! Staged application input, not automatic physical-memory discovery.

use super::{Counters, Pipe};
use crate::picture::deposit::admission::Share;
use crate::sysconf::conf_value;
use std::{sync::Arc, time::Duration};

/// All bounds are supplied by the application qualification profile. No RSS,
/// queue ceiling or missing owner is interpreted as available physical memory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Configured {
    envelope: usize,
    others: usize,
    bytes: usize,
    entries: usize,
    interval: Duration,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Profile {
    #[default]
    Unconfigured,
    Invalid,
    Staged(Configured),
}

impl Profile {
    pub(crate) fn parse(conf: &str) -> Self {
        match conf_value(None, conf, "f2.stage").as_deref() {
            Some("configured-evaluation") => Configured::parse(conf).map_or(Self::Invalid, Self::Staged),
            _ => Self::Unconfigured,
        }
    }
}

impl Configured {
    pub(crate) fn parse(conf: &str) -> Option<Self> {
        let number = |key| conf_value(None, conf, key)?.parse::<usize>().ok().filter(|n| *n > 0);
        // Shared includes newest/STOP and independent clones. Copies includes
        // records/history, analysis jobs/results and scratch. Old tap plans stay
        // inside the single Funding ledger, never in a second allowance here.
        let others = [
            "f2.window_graphics_bytes",
            "f2.dma_bytes",
            "f2.shared_bytes",
            "f2.copies_analysis_bytes",
            "f2.runtime_bytes",
        ]
        .into_iter()
        .try_fold(0usize, |sum, key| sum.checked_add(number(key)?))?;
        let input = Self {
            envelope: number("f2.envelope_bytes")?,
            others,
            bytes: number("f2.tap_bytes")?,
            entries: number("f2.tap_entries")?,
            interval: Duration::from_nanos(u64::try_from(number("f2.min_cycle_ns")?).ok()?),
        };
        (input.others.checked_add(input.bytes)? <= input.envelope && input.interval <= Duration::from_secs(1))
            .then_some(input)
    }
}

/// The process OnceLock retains the staged accounting permit before the first source admission.
pub(super) fn application(sw: &crate::sysconf::Switches, counters: &'static Counters) -> Pipe {
    let cores = sw.cores;
    let workers = cores.little_count().unwrap_or(super::WORKERS);
    let pipe = Pipe::start(workers, cores.little, counters);
    super::admission::model_pool(&pipe, workers);
    if sw.f2_funding != Profile::Unconfigured {
        pipe.slot.require_tap_funding();
        let funded = if let Profile::Staged(input) = sw.f2_funding {
            Share::checked(input.envelope, input.others, input.bytes, input.entries, Arc::new(input))
                .and_then(|share| pipe.fund_taps(share, input.interval))
                .is_some()
        } else {
            false
        };
        if !funded {
            eprintln!("oos-ui: F2 configured-evaluation funding refused");
        }
    }
    pipe
}

#[cfg(test)]
pub(crate) const TEST_PROFILE: &str = "f2.stage=configured-evaluation\nf2.envelope_bytes=268435456\nf2.tap_bytes=134217728\nf2.tap_entries=512\nf2.min_cycle_ns=500000\nf2.window_graphics_bytes=1\nf2.dma_bytes=1\nf2.shared_bytes=1\nf2.copies_analysis_bytes=1\nf2.runtime_bytes=1\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_input_requires_every_owner_and_checked_partition() {
        let conf = TEST_PROFILE.to_string();
        assert!(matches!(Profile::parse(&conf), Profile::Staged(_)));
        assert_eq!(Profile::parse(""), Profile::Unconfigured);
        assert_eq!(
            Profile::parse(include_str!("../../../rootfs/initramfs/etc/oos/system.conf")),
            Profile::Unconfigured
        );
        for line in conf.lines() {
            let missing = conf.lines().filter(|s| *s != line).collect::<Vec<_>>().join("\n");
            assert!(!matches!(Profile::parse(&missing), Profile::Staged(_)), "missing {line}");
        }
        for (key, value) in [
            ("envelope_bytes", "1"),
            ("dma_bytes", "18446744073709551615"),
            ("min_cycle_ns", "1000000001"),
            ("tap_entries", "0"),
        ] {
            let bad = format!("f2.{key}={value}\n{conf}");
            assert_eq!(Profile::parse(&bad), Profile::Invalid, "{key}");
        }
    }
}
