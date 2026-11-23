// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The fork's switches: the `OOS_*` overrides in the environment, the fork's keys of system.conf (`fork_text`) and
//! the platform, read once at start (`Switches::read`) and passed in. `Switches::default()` is the empty
//! configuration: what a scope without a system.conf and without overrides does. Tests use it.

use super::conf_value;
use crate::pacing::{ClockMode, FlipDivisor};
use crate::sparse::Interp;
use std::ffi::OsString;

mod cpu;
mod dump;
mod source;
pub(crate) use cpu::GpuCpu;
pub(crate) use dump::BatchDump;
use source::SourceOverhead;

/// An environment lookup: the variable's value as the OS has it, None when it is not set.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The fork's switches of one process.
#[derive(Clone, Debug, PartialEq)]
pub struct Switches {
    /// OOS_DEMO_LOOP: channel mask the demo generator's output is looped back into (0: none)
    pub demo_loop: u32,
    /// live compression (`compress::enabled_from`)
    pub compress: bool,
    /// the big and little cores (`placement::Cores`): batch workers, background threads, placement
    pub cores: crate::placement::Cores,
    /// OOS_FIXED_TIME: the clock of the UI stands at these seconds since 1970, for host test runs whose pictures must
    /// not change with the time of day (tools/layout-check.sh)
    pub fixed_time: Option<f64>,
    /// interpolation of sparse frames (`sparse::interp_from`)
    pub interp: Interp,
    /// Thread/IRQ placement, RUN clock floors and frame pacing.
    pub perf: Performance,
    pub(crate) source: SourceOverhead,
    /// IPv6 (`net6::enabled_from`)
    pub ipv6: bool,
    /// the board's own switches
    pub board: crate::board::BoardSwitches,
    pub memory: crate::system::memory::Limits,
    pub(crate) f2_funding: crate::drawpipe::funding::Profile,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Performance {
    place: bool,
    floors: crate::system::clocks::Config,
    pub(crate) pacing_clock: ClockMode,
    pub(crate) flip_divisor: FlipDivisor,
    pub(crate) gpu_cpu: GpuCpu,
}

impl Performance {
    pub fn place(&self) -> bool {
        self.place
    }
    pub fn floors(&self) -> crate::system::clocks::Config {
        self.floors
    }
}

/// The value of an `OOS_*` variable: None when it is not set or not text.
pub fn text(env: Env, key: &str) -> Option<String> {
    env(key).and_then(|v| v.into_string().ok())
}

/// An `OOS_*` flag: empty or 0 off, any other value on.
pub fn flag(env: Env, key: &str) -> Option<&'static str> {
    text(env, key).map(|v| if v.is_empty() || v == "0" { "off" } else { "on" })
}

fn report_obsolete_keys(conf: &str, mut report: impl FnMut(&'static str)) {
    let present = |key: &&str| conf_value(None, conf, key).is_some();
    ["acq.pipeline", "acq.stop_slot"].into_iter().filter(present).for_each(&mut report);
    if let Some(key) = ["acq.play", "acq.play.inferred"].into_iter().find(present) {
        report(key);
    }
}

impl Switches {
    /// The switches from an environment lookup, the fork's system.conf text (`fork_text`), the settings file alone
    /// and the cores.
    pub fn parse(env: Env, conf: &str, settings: &str, cores: crate::placement::Cores) -> Self {
        report_obsolete_keys(conf, |key| eprintln!("oos-ui: ignoring obsolete system.conf key {key}"));
        Switches {
            demo_loop: text(env, "OOS_DEMO_LOOP").and_then(|v| v.parse().ok()).unwrap_or(0),
            compress: crate::compress::enabled_from(flag(env, "OOS_COMPRESS"), conf),
            cores,
            fixed_time: text(env, "OOS_FIXED_TIME").and_then(|v| v.parse().ok()),
            interp: crate::sparse::interp_from(text(env, "OOS_INTERP").as_deref(), conf),
            perf: Performance {
                gpu_cpu: GpuCpu::parse(env, conf),
                place: conf_value(flag(env, "OOS_PLACE"), conf, "perf.place").as_deref() == Some("on"),
                floors: crate::system::clocks::Config::parse(conf),
                pacing_clock: match conf_value(text(env, "OOS_PACING_CLOCK").as_deref(), conf, "pacing.clock")
                    .as_deref()
                {
                    Some("timer") => ClockMode::Timer,
                    Some("flip") | None => ClockMode::Flip,
                    Some(value) => {
                        eprintln!("oos-ui: invalid pacing.clock {value:?}; using default flip");
                        ClockMode::default()
                    }
                },
                flip_divisor: match conf_value(
                    text(env, "OOS_PACING_FLIP_DIVISOR").as_deref(),
                    conf,
                    "pacing.flip_divisor",
                )
                .as_deref()
                {
                    Some("2") => FlipDivisor::Two,
                    Some("3") | None => FlipDivisor::Three,
                    Some("4") => FlipDivisor::Four,
                    Some(value) => {
                        eprintln!("oos-ui: invalid pacing.flip_divisor {value:?}; using default 3");
                        FlipDivisor::default()
                    }
                },
            },
            source: SourceOverhead::parse(env, conf, |key, value, default| {
                eprintln!("oos-ui: invalid {key} {value:?}; using default {default}");
            }),
            ipv6: crate::net6::enabled_from(flag(env, "OOS_IPV6"), conf),
            board: crate::board::BoardSwitches::parse(env, conf, settings),
            memory: crate::system::memory::Limits::parse(conf),
            f2_funding: crate::drawpipe::funding::Profile::parse(conf),
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
        assert_eq!((s.demo_loop, s.compress, s.interp), (0, false, Interp::Exact));
        assert!(!s.perf.place() && !s.ipv6);
        assert_eq!(s.board, crate::board::BoardSwitches::parse(&|_| None, "", ""));
    }

    #[test]
    fn obsolete_pipeline_key_is_ignored_with_other_settings_intact() {
        let conf = "acq.pipeline = frames # old user setting\nntp = on\nacq.interp = fast\n";
        let image = "acq.pipeline = batch\n";
        let (system, errors) = super::super::SysConf::parse(conf);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(system.ntp);
        assert!(!system.text().contains("acq.pipeline"));
        let merged = super::super::fork_text_of(conf, image);
        let switches = Switches::parse(&|_| None, &merged, conf, Default::default());
        let current = Switches::parse(&|_| None, "acq.interp = fast", "", Default::default());
        assert_eq!(switches, current);
    }

    #[test]
    fn obsolete_play_keys_are_ignored_with_one_notice() {
        let conf = "acq.play = stock # old user setting\nacq.play.inferred = on\nntp = on\n";
        let (system, errors) = super::super::SysConf::parse(conf);
        assert!(errors.is_empty(), "{errors:?}");
        assert!(system.ntp);
        assert!(!system.text().contains("acq.play"));

        let merged = super::super::fork_text_of(conf, "acq.play = raw\n");
        let mut reports = Vec::new();
        report_obsolete_keys(&merged, |key| reports.push(key));
        assert_eq!(reports, ["acq.play"]);
        reports.clear();
        report_obsolete_keys("acq.play.inferred = on", |key| reports.push(key));
        assert_eq!(reports, ["acq.play.inferred"]);
        let switches = Switches::parse(&|_| None, &merged, conf, Default::default());
        assert_eq!(switches, Switches::default());
    }

    #[test]
    fn obsolete_stop_slot_is_ignored_with_one_notice() {
        for slot in ["zero", "alternate", "invalid", ""] {
            let conf = format!("acq.stop_slot = {slot} # old user setting\nntp = on\n");
            let (system, errors) = super::super::SysConf::parse(&conf);
            assert!(errors.is_empty(), "{errors:?}");
            assert!(system.ntp);
            assert!(!system.text().contains("acq.stop_slot"));

            let merged = super::super::fork_text_of(&conf, "acq.stop_slot = zero\n");
            let mut reports = Vec::new();
            report_obsolete_keys(&merged, |key| reports.push(key));
            assert_eq!(reports, ["acq.stop_slot"]);
            assert_eq!(Switches::parse(&|_| None, &merged, &conf, Default::default()), Switches::default());
        }
    }

    #[test]
    fn pacing_clock_is_a_startup_switch_with_flip_as_default() {
        let parse = |value: Option<&str>, conf| {
            Switches::parse(
                &|key| (key == "OOS_PACING_CLOCK").then_some(value).flatten().map(OsString::from),
                conf,
                "",
                crate::placement::Cores::default(),
            )
            .perf
            .pacing_clock
        };
        assert_eq!(Switches::default().perf.pacing_clock, ClockMode::Flip);
        assert_eq!(parse(None, "pacing.clock = timer"), ClockMode::Timer);
        assert_eq!(parse(Some("flip"), "pacing.clock = timer"), ClockMode::Flip);
        assert_eq!(parse(Some("invalid"), "pacing.clock = timer"), ClockMode::Flip);
    }

    #[test]
    fn flip_divisor_accepts_two_through_four_and_defaults_to_three() {
        let parse = |value: Option<&str>, conf| {
            let env = |key: &str| (key == "OOS_PACING_FLIP_DIVISOR").then_some(value).flatten().map(OsString::from);
            Switches::parse(&env, conf, "", Default::default()).perf.flip_divisor
        };
        assert_eq!(Switches::default().perf.flip_divisor, FlipDivisor::Three);
        assert_eq!(parse(None, "pacing.flip_divisor = 2 # comment"), FlipDivisor::Two);
        assert_eq!(parse(None, "pacing.flip_divisor = 3"), FlipDivisor::Three);
        assert_eq!(parse(None, "pacing.flip_divisor = 4"), FlipDivisor::Four);
        assert_eq!(parse(Some("4"), "pacing.flip_divisor = 2"), FlipDivisor::Four);
        for invalid in ["", "1", "5", "3.0", "-3"] {
            assert_eq!(parse(Some(invalid), "pacing.flip_divisor = 2"), FlipDivisor::Three);
        }
    }

    #[test]
    fn environment_overrides_system_conf() {
        let env = |k: &str| match k {
            "OOS_DEMO_LOOP" => Some("15".into()),
            "OOS_COMPRESS" | "OOS_PLACE" => Some("1".into()),
            "OOS_FIXED_TIME" => Some("946684800".into()),
            _ => None,
        };
        let conf = "net.ipv6 = on\nacq.interp = fast\n";
        let cores = crate::placement::Cores { big: 0b11_0000, little: 0b1111 };
        let s = Switches::parse(&env, conf, "", cores);
        assert_eq!((s.demo_loop, s.compress, s.interp), (15, true, Interp::Fast));
        assert!(s.perf.place() && s.ipv6);
        assert_eq!((s.cores, s.fixed_time), (cores, Some(946_684_800.0)));
    }

    // Break it catches: a presence switch read as text, so a value that is not UTF-8 counts as unset.
    #[test]
    fn presence_switches_count_any_os_value() {
        use std::os::unix::ffi::OsStringExt;
        let env = |k: &str| match k {
            "OOS_FRAME_HEADER" | "OOS_PREARM" | "OOS_COMPRESS" => Some(OsString::from_vec(vec![0xff])),
            _ => None,
        };
        let s = Switches::parse(&env, "", "", crate::placement::Cores::default());
        assert!(s.board.header_probe.on() && s.board.prearm, "presence only");
        assert!(!s.compress, "a value switch takes text only, as std::env::var did");
    }
}
