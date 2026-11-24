// SPDX-License-Identifier: GPL-2.0-or-later

use super::{conf_value, text, Env};
use crate::drawpipe::budget::default_work_budget;
use std::fmt;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Capture {
    mode: CaptureMode,
    inferred: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum CaptureMode {
    #[default]
    Full,
    Auto,
    Explicit {
        peak: bool,
        factor: u32,
    },
}

impl Capture {
    fn parse(conf: &str, report: &mut impl FnMut(&str, &str, &str)) -> Self {
        let inferred = match conf_value(None, conf, "acq.capture.inferred").as_deref() {
            Some("on") => true,
            Some("off") | None => false,
            Some(value) => {
                report("acq.capture.inferred", value, "off");
                false
            }
        };
        let value = conf_value(None, conf, "acq.capture");
        let mode = match value.as_deref() {
            None | Some("full") => CaptureMode::Full,
            Some("auto") => CaptureMode::Auto,
            Some(value) => value
                .split_once(':')
                .and_then(|(mode, factor)| {
                    let factor = factor.parse::<u32>().ok()?;
                    if !matches!(factor, 2 | 4 | 8 | 16 | 32 | 64) {
                        return None;
                    }
                    matches!(mode, "decimate" | "peak")
                        .then_some(CaptureMode::Explicit { peak: mode == "peak", factor })
                })
                .unwrap_or_else(|| {
                    report("acq.capture", value, "full");
                    CaptureMode::Full
                }),
        };
        Self { mode, inferred }
    }

    pub(crate) fn reduction(self) -> Option<(bool, u32, bool)> {
        let CaptureMode::Explicit { peak, factor } = self.mode else { return None };
        Some((peak, factor, self.inferred))
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn auto(self) -> bool {
        self.mode == CaptureMode::Auto
    }
}

impl fmt::Display for Capture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.mode {
            CaptureMode::Explicit { peak, factor } => write!(f, "{}:{factor}", if peak { "peak" } else { "decimate" }),
            CaptureMode::Full => f.write_str("full"),
            CaptureMode::Auto => f.write_str("auto"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SourceOverhead {
    batch_settle: bool,
    fifo: bool,
    work_budget: Option<Duration>,
    capture: Capture,
    play_window: bool,
}

impl Default for SourceOverhead {
    fn default() -> Self {
        Self { batch_settle: true, fifo: true, work_budget: None, capture: Capture::default(), play_window: true }
    }
}

impl SourceOverhead {
    pub(super) fn parse(env: Env, conf: &str, mut report: impl FnMut(&str, &str, &str)) -> Self {
        let mut enabled = |key, var, default, alternate| match conf_value(text(env, var).as_deref(), conf, key) {
            Some(value) if value == alternate => false,
            Some(value) if value != default => {
                report(key, &value, default);
                true
            }
            _ => true,
        };
        let batch_settle = enabled("acq.write_gap", "OOS_ACQ_WRITE_GAP", "0", "10");
        let fifo = enabled("acq.sched", "OOS_ACQ_SCHED", "fifo", "other");
        let play_window = match conf_value(None, conf, "acq.play_window").as_deref() {
            Some("on") | None => true,
            Some("off") => false,
            Some(value) => {
                report("acq.play_window", value, "on");
                true
            }
        };
        Self {
            batch_settle,
            fifo,
            work_budget: conf_value(text(env, "OOS_ACQ_WORK_BUDGET_MS").as_deref(), conf, "acq.work_budget_ms")
                .and_then(|value| {
                    if let Ok(ms @ 4..=20) = value.parse::<u64>() {
                        return Some(Duration::from_millis(ms));
                    }
                    report("acq.work_budget_ms", &value, "36% of the picture period");
                    None
                }),
            capture: Capture::parse(conf, &mut report),
            play_window,
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn batch_settle(self) -> bool {
        self.batch_settle
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn fifo(self) -> bool {
        self.fifo
    }

    pub(crate) fn work_budget(self) -> Duration {
        self.work_budget.unwrap_or_else(|| default_work_budget(crate::pacing::picture_period()))
    }

    pub(crate) fn capture(self) -> Capture {
        self.capture
    }

    pub(crate) fn play_window(self) -> bool {
        self.play_window
    }
}

impl fmt::Display for SourceOverhead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let gap = if self.batch_settle { 0 } else { 10 };
        let sched = if self.fifo { "fifo" } else { "other" };
        write!(
            f,
            "acq.write_gap={gap} acq.sched={sched} acq.work_budget_ms={} acq.capture.requested={} acq.capture.inferred={} acq.play_window={}",
            self.work_budget().as_millis(),
            self.capture,
            if self.capture.inferred { "on" } else { "off" },
            if self.play_window() { "on" } else { "off" },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sysconf::Switches;

    const DEFAULTS: &str = "acq.write_gap = 0\nacq.sched = fifo\n";
    const LEGACY: &str = "acq.write_gap = 10\nacq.sched = other\n";
    const DISABLED: SourceOverhead = SourceOverhead {
        batch_settle: false,
        fifo: false,
        work_budget: None,
        capture: Capture { mode: CaptureMode::Full, inferred: false },
        play_window: true,
    };

    #[test]
    fn source_switch_values_and_defaults() {
        assert_eq!(Switches::default().source, SourceOverhead::default());
        assert_eq!(Switches::parse(&|_| None, "", "", Default::default()).source, SourceOverhead::default());
        assert_eq!(Switches::parse(&|_| None, DEFAULTS, "", Default::default()).source, SourceOverhead::default());
        assert_eq!(Switches::parse(&|_| None, LEGACY, "", Default::default()).source, DISABLED);
    }

    #[test]
    fn source_switch_environment_supersedes_system_conf() {
        for (conf, gap, sched, expected) in
            [(DEFAULTS, "10", "other", DISABLED), (LEGACY, "0", "fifo", SourceOverhead::default())]
        {
            let env = |key: &str| match key {
                "OOS_ACQ_WRITE_GAP" => Some(gap.into()),
                "OOS_ACQ_SCHED" => Some(sched.into()),
                _ => None,
            };
            assert_eq!(Switches::parse(&env, conf, "", Default::default()).source, expected);
        }
    }

    #[test]
    fn invalid_source_switches_are_reported_and_use_defaults() {
        let conf = "acq.write_gap = 5\nacq.sched = idle";
        let mut reported = Vec::new();
        let source = SourceOverhead::parse(&|_| None, conf, |key, value, default| {
            reported.push(format!("{key} {value} -> {default}"));
        });
        assert_eq!(source, SourceOverhead::default());
        assert_eq!(reported, ["acq.write_gap 5 -> 0", "acq.sched idle -> fifo"]);
    }

    #[test]
    fn play_window_is_on_unless_explicitly_disabled() {
        for (conf, expected, errors) in [
            ("", true, 0),
            ("acq.play_window = off", false, 0),
            ("acq.play_window = on", true, 0),
            ("acq.play_window = no", true, 1),
        ] {
            let mut reports = Vec::new();
            let switches = SourceOverhead::parse(&|_| None, conf, |key, value, fallback| {
                reports.push(format!("{key} {value} -> {fallback}"));
            });
            assert_eq!(switches.play_window(), expected);
            assert_eq!(reports.len(), errors);
        }
    }

    #[test]
    fn work_budget_accepts_only_integer_milliseconds_in_range() {
        assert_eq!(Switches::default().source.work_budget(), Duration::from_millis(12));
        for ms in [4, 12, 20] {
            let conf = format!("acq.work_budget_ms = {ms} # comment");
            assert_eq!(
                Switches::parse(&|_| None, &conf, "", Default::default()).source.work_budget(),
                Duration::from_millis(ms)
            );
        }
        for value in ["", "-1", "0", "3", "21", "4.5", "18446744073709551616"] {
            let mut reports = Vec::new();
            let source =
                SourceOverhead::parse(&|_| None, &format!("acq.work_budget_ms = {value}"), |key, invalid, default| {
                    reports.push((key.to_string(), invalid.to_string(), default.to_string()));
                });
            assert_eq!(source.work_budget(), Duration::from_millis(12));
            assert_eq!(reports, [("acq.work_budget_ms".into(), value.into(), "36% of the picture period".into())]);
        }
    }

    #[test]
    fn work_budget_environment_supersedes_system_conf() {
        let source = Switches::parse(
            &|key| (key == "OOS_ACQ_WORK_BUDGET_MS").then(|| "17".into()),
            "acq.work_budget_ms = 4",
            "",
            Default::default(),
        )
        .source;
        assert_eq!(source.work_budget(), Duration::from_millis(17));
    }

    #[test]
    fn capture_requires_an_explicit_system_conf_value() {
        let auto = Switches::parse(&|_| None, "acq.capture = auto", "", Default::default()).source.capture();
        assert!(auto.auto());
        assert_eq!(auto.to_string(), "auto");
        assert!(!Capture::default().auto());
        for (peak, mode) in [(false, "decimate"), (true, "peak")] {
            for factor in [2, 4, 8, 16, 32, 64] {
                let conf = format!("acq.capture = {mode}:{factor}\nacq.capture.inferred = on");
                let capture = Switches::parse(&|_| None, &conf, "", Default::default()).source.capture();
                assert_eq!(capture.reduction(), Some((peak, factor, true)));
                assert_eq!(capture.to_string(), format!("{mode}:{factor}"));
            }
        }
        let capture = Switches::parse(&|_| Some("peak:2".into()), "", "", Default::default()).source.capture();
        assert_eq!(capture.reduction(), None);
    }

    #[test]
    fn malformed_capture_values_fail_closed() {
        for value in ["", "peak", "peak:3", "peak:128", "decimate:1", "full:2", "other:2"] {
            let mut reports = Vec::new();
            let conf = format!("acq.capture = {value}\nacq.capture.inferred = true");
            let capture = Capture::parse(&conf, &mut |key, invalid, fallback| {
                reports.push(format!("{key} {invalid} -> {fallback}"));
            });
            assert_eq!(capture, Capture::default());
            assert_eq!(reports, ["acq.capture.inferred true -> off", &format!("acq.capture {value} -> full")]);
        }
        assert_eq!(Capture::parse("acq.capture = decimate:2", &mut |_, _, _| {}).reduction(), Some((false, 2, false)));
    }
}
