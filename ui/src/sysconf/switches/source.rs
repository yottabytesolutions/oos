// SPDX-License-Identifier: GPL-2.0-or-later

use super::{conf_value, text, Env};
use crate::drawpipe::budget::default_work_budget;
use std::fmt;
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum StopSlot {
    #[default]
    Zero,
    Alternate,
}

impl StopSlot {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Zero => "zero",
            Self::Alternate => "alternate",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Capture {
    reduction: Option<(bool, u32)>,
    inferred: bool,
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
        let reduction = conf_value(None, conf, "acq.capture").and_then(|value| {
            if value == "full" {
                return None;
            }
            let request = value.split_once(':').and_then(|(mode, factor)| {
                let factor = factor.parse::<u32>().ok()?;
                if !matches!(factor, 2 | 4 | 8 | 16 | 32 | 64) {
                    return None;
                }
                match mode {
                    "decimate" => Some((false, factor)),
                    "peak" => Some((true, factor)),
                    _ => None,
                }
            });
            if request.is_none() {
                report("acq.capture", &value, "full");
            }
            request
        });
        Self { reduction, inferred }
    }

    pub(crate) fn reduction(self) -> Option<(bool, u32, bool)> {
        self.reduction.map(|(peak, factor)| (peak, factor, self.inferred))
    }
}

impl fmt::Display for Capture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.reduction {
            Some((peak, factor)) => write!(f, "{}:{factor}", if peak { "peak" } else { "decimate" }),
            None => f.write_str("full"),
        }
    }
}

fn stock_play(conf: &str, report: &mut impl FnMut(&str, &str, &str)) -> Option<bool> {
    let inferred = match conf_value(None, conf, "acq.play.inferred").as_deref() {
        Some("on") => true,
        Some("off") | None => false,
        Some(value) => {
            report("acq.play.inferred", value, "off");
            false
        }
    };
    match conf_value(None, conf, "acq.play").as_deref() {
        Some("stock") => Some(inferred),
        Some("raw") | None => None,
        Some(value) => {
            report("acq.play", value, "raw");
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SourceOverhead {
    batch_settle: bool,
    fifo: bool,
    work_budget: Option<Duration>,
    stop_slot: StopSlot,
    capture: Capture,
    stock_play: Option<bool>,
}

impl Default for SourceOverhead {
    fn default() -> Self {
        Self {
            batch_settle: true,
            fifo: true,
            work_budget: None,
            stop_slot: StopSlot::Zero,
            capture: Capture::default(),
            stock_play: None,
        }
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
        Self {
            batch_settle: enabled("acq.write_gap", "OOS_ACQ_WRITE_GAP", "0", "10"),
            fifo: enabled("acq.sched", "OOS_ACQ_SCHED", "fifo", "other"),
            work_budget: conf_value(text(env, "OOS_ACQ_WORK_BUDGET_MS").as_deref(), conf, "acq.work_budget_ms")
                .and_then(|value| {
                    if let Ok(ms @ 4..=20) = value.parse::<u64>() {
                        return Some(Duration::from_millis(ms));
                    }
                    report("acq.work_budget_ms", &value, "36% of the picture period");
                    None
                }),
            stop_slot: match conf_value(None, conf, "acq.stop_slot").as_deref() {
                Some("alternate") => StopSlot::Alternate,
                Some("zero") | None => StopSlot::Zero,
                Some(value) => {
                    report("acq.stop_slot", value, "zero");
                    StopSlot::Zero
                }
            },
            capture: Capture::parse(conf, &mut report),
            stock_play: stock_play(conf, &mut report),
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

    pub(crate) fn stop_slot(self) -> StopSlot {
        self.stop_slot
    }

    pub(crate) fn capture(self) -> Capture {
        self.capture
    }

    /// A stock request and its inferred-value opt-in.
    pub(crate) fn stock_play(self) -> Option<bool> {
        self.stock_play
    }
}

impl fmt::Display for SourceOverhead {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let gap = if self.batch_settle { 0 } else { 10 };
        let sched = if self.fifo { "fifo" } else { "other" };
        let play = if self.stock_play.is_some() { "stock" } else { "raw" };
        let inferred = if self.stock_play == Some(true) { "on" } else { "off" };
        write!(
            f,
            "acq.write_gap={gap} acq.sched={sched} acq.work_budget_ms={} acq.capture.requested={} acq.capture.inferred={} acq.play.requested={play} acq.play.inferred={inferred}",
            self.work_budget().as_millis(),
            self.capture,
            if self.capture.inferred { "on" } else { "off" },
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
        stop_slot: StopSlot::Zero,
        capture: Capture { reduction: None, inferred: false },
        stock_play: None,
    };

    #[test]
    fn stock_play_parses_only_explicit_system_conf_opt_ins() {
        for (conf, expected) in [
            ("", None),
            ("acq.play.inferred = on", None),
            ("acq.play = raw\nacq.play.inferred = on", None),
            ("acq.play = stock", Some(false)),
            ("acq.play = stock\nacq.play.inferred = on", Some(true)),
        ] {
            let sw = Switches::parse(&|_| Some("stock".into()), conf, "", Default::default());
            assert_eq!(sw.source.stock_play(), expected);
        }
        let mut reports = Vec::new();
        let invalid = stock_play("acq.play = linear\nacq.play.inferred = yes", &mut |key, _, _| {
            reports.push(key.to_string());
        });
        assert_eq!(invalid, None);
        assert_eq!(reports, ["acq.play.inferred", "acq.play"]);
    }

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
        let conf = "acq.write_gap = 5\nacq.sched = idle\nacq.stop_slot = other\n";
        let mut reported = Vec::new();
        let source = SourceOverhead::parse(&|_| None, conf, |key, value, default| {
            reported.push(format!("{key} {value} -> {default}"));
        });
        assert_eq!(source, SourceOverhead::default());
        assert_eq!(reported, ["acq.write_gap 5 -> 0", "acq.sched idle -> fifo", "acq.stop_slot other -> zero"]);
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
