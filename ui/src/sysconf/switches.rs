// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The fork's switches: the `OOS_*` overrides in the environment, the fork's keys of system.conf (`fork_text`) and
//! the platform, read once at start (`Switches::read`) and passed in. `Switches::default()` is the empty
//! configuration: what a scope without a system.conf and without overrides does. Tests use it.

use crate::drawpipe::{conf_value, Mode};
use crate::sparse::Interp;
use std::ffi::OsString;

/// An environment lookup: the variable's value as the OS has it, None when it is not set.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The fork's switches of one process.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Switches {
    /// OOS_DEMO_LOOP: channel mask the demo generator's output is looped back into (0: none)
    pub demo_loop: u32,
    /// live compression (`compress::enabled_from`)
    pub compress: bool,
    /// pipeline mode (`drawpipe::mode_from`)
    pub pipeline: Mode,
    /// the big and little cores (`placement::Cores`): batch workers, background threads, placement
    pub cores: crate::placement::Cores,
    /// OOS_FIXED_TIME: the clock of the UI stands at these seconds since 1970, for host test runs whose pictures must
    /// not change with the time of day (tools/layout-check.sh)
    pub fixed_time: Option<f64>,
    /// interpolation of sparse frames (`sparse::interp_from`)
    pub interp: Interp,
    /// raw deep-record measurements (`deepmeas::enabled_from`)
    pub deep_raw: bool,
    /// thread placement on the CPUs (`placement::place`)
    pub place: bool,
    /// IPv6 (`net6::enabled_from`)
    pub ipv6: bool,
    /// the auto set-up reads levels with the measure unit (`autoset.levels`)
    pub autoset_unit_levels: bool,
    /// the board's own switches
    pub board: crate::board::BoardSwitches,
    pub memory: crate::system::memory::Limits,
}

/// The value of an `OOS_*` variable: None when it is not set or not text.
pub fn text(env: Env, key: &str) -> Option<String> {
    env(key).and_then(|v| v.into_string().ok())
}

/// An `OOS_*` flag: empty or 0 off, any other value on.
pub fn flag(env: Env, key: &str) -> Option<&'static str> {
    text(env, key).map(|v| if v.is_empty() || v == "0" { "off" } else { "on" })
}

impl Switches {
    /// The switches from an environment lookup, the fork's system.conf text (`fork_text`), the settings file alone
    /// and the cores.
    pub fn parse(env: Env, conf: &str, settings: &str, cores: crate::placement::Cores) -> Self {
        let levels = conf_value(text(env, "OOS_AUTOSET_LEVELS").as_deref(), conf, "autoset.levels");
        Switches {
            demo_loop: text(env, "OOS_DEMO_LOOP").and_then(|v| v.parse().ok()).unwrap_or(0),
            compress: crate::compress::enabled_from(flag(env, "OOS_COMPRESS"), conf),
            pipeline: crate::drawpipe::mode_from(text(env, "OOS_PIPELINE").as_deref(), conf),
            cores,
            fixed_time: text(env, "OOS_FIXED_TIME").and_then(|v| v.parse().ok()),
            interp: crate::sparse::interp_from(text(env, "OOS_INTERP").as_deref(), conf),
            deep_raw: crate::deepmeas::enabled_from(flag(env, "OOS_DEEP_RAW"), conf),
            place: conf_value(flag(env, "OOS_PLACE"), conf, "perf.place").as_deref() == Some("on"),
            ipv6: crate::net6::enabled_from(flag(env, "OOS_IPV6"), conf),
            autoset_unit_levels: levels.as_deref() != Some("records"),
            board: crate::board::BoardSwitches::parse(env, conf, settings),
            memory: crate::system::memory::Limits::parse(conf),
        }
    }

    /// The switches of this process: the environment, system.conf and the CPUs. Called once at start.
    pub fn read() -> Self {
        let settings = std::fs::read_to_string(super::default_path()).unwrap_or_default();
        let image = std::fs::read_to_string(super::IMAGE_PATH).unwrap_or_default();
        let conf = super::fork_text_of(&settings, &image);
        Self::parse(&|k| std::env::var_os(k), &conf, &settings, crate::placement::Cores::read())
    }
}

impl Default for Switches {
    fn default() -> Self {
        Self::parse(&|_| None, "", "", crate::placement::Cores::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_configuration_is_the_default() {
        let s = Switches::default();
        assert_eq!((s.demo_loop, s.compress, s.pipeline, s.interp), (0, false, Mode::Frames, Interp::Exact));
        assert!(s.deep_raw && !s.place && !s.ipv6 && s.autoset_unit_levels);
        assert_eq!(s.board, crate::board::BoardSwitches::parse(&|_| None, "", ""));
    }

    #[test]
    fn environment_overrides_system_conf() {
        let env = |k: &str| match k {
            "OOS_DEMO_LOOP" => Some("15".into()),
            "OOS_COMPRESS" | "OOS_PLACE" => Some("1".into()),
            "OOS_DEEP_RAW" => Some("0".into()),
            "OOS_PIPELINE" => Some("batch".into()),
            "OOS_FIXED_TIME" => Some("946684800".into()),
            _ => None,
        };
        let conf = "net.ipv6 = on\nacq.interp = fast\nautoset.levels = records\nmeas.deep_raw = on\n";
        let cores = crate::placement::Cores { big: 0b11_0000, little: 0b1111 };
        let s = Switches::parse(&env, conf, "", cores);
        assert_eq!((s.demo_loop, s.compress, s.pipeline, s.interp), (15, true, Mode::Batch, Interp::Fast));
        assert!(!s.deep_raw && s.place && s.ipv6 && !s.autoset_unit_levels);
        assert_eq!((s.cores, s.fixed_time), (cores, Some(946_684_800.0)));
    }

    // Break it catches: a presence switch read as text, so a value that is not UTF-8 counts as unset.
    #[cfg(unix)]
    #[test]
    fn presence_switches_count_any_os_value() {
        use std::os::unix::ffi::OsStringExt;
        let env = |k: &str| match k {
            "OOS_FRAME_HEADER" | "OOS_PREARM" | "OOS_COMPRESS" => Some(OsString::from_vec(vec![0xff])),
            _ => None,
        };
        let s = Switches::parse(&env, "", "", crate::placement::Cores::default());
        assert!(s.board.header_probe && s.board.prearm, "presence only");
        assert!(!s.compress, "a value switch takes text only, as std::env::var did");
    }
}
