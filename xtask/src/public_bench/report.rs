// SPDX-License-Identifier: GPL-2.0-or-later

use super::{settings::Settings, BenchmarkOptions, Error, SETTLE, WINDOW};
use crate::unit_tools::report;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, io, path::Path, time::Duration};

const SCHEMA: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Metric {
    PicturesPerS,
    AcquiredPerS,
    ShownPerS,
    DroppedPerS,
    ReportedFps,
    StepMs,
    RenderMs,
    PresentMs,
    OpcHostMedianMs,
    OpcHostMaxMs,
}

impl Metric {
    const ALL: [Self; 10] = [
        Self::PicturesPerS,
        Self::AcquiredPerS,
        Self::ShownPerS,
        Self::DroppedPerS,
        Self::ReportedFps,
        Self::StepMs,
        Self::RenderMs,
        Self::PresentMs,
        Self::OpcHostMedianMs,
        Self::OpcHostMaxMs,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::PicturesPerS => "pictures/s",
            Self::AcquiredPerS => "acquired/s",
            Self::ShownPerS => "shown/s",
            Self::DroppedPerS => "dropped/s",
            Self::ReportedFps => "reported fps",
            Self::StepMs => "step ms",
            Self::RenderMs => "render ms",
            Self::PresentMs => "present ms",
            Self::OpcHostMedianMs => "OPC host RTT median ms",
            Self::OpcHostMaxMs => "OPC host RTT max ms",
        }
    }
}

pub(super) type Values = BTreeMap<Metric, Option<f64>>;

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Sample {
    pub(super) settings: Settings,
    pub(super) elapsed_s: f64,
    pub(super) values: Values,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Spread {
    pub(super) median: f64,
    min: f64,
    pub(super) max: f64,
    observations: usize,
}

impl Spread {
    pub(super) fn calculate(mut values: Vec<f64>) -> Option<Self> {
        values.sort_unstable_by(f64::total_cmp);
        let observations = values.len();
        let (&min, &max) = values.first().zip(values.last())?;
        let median = f64::midpoint(values[(observations - 1) / 2], values[observations / 2]);
        Some(Self { median, min, max, observations })
    }

    fn display(&self) -> String {
        format!("{:.3} [{:.3}, {:.3}] n={}", self.median, self.min, self.max, self.observations)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ViewReport {
    name: String,
    pub(super) samples: Vec<Sample>,
    summary: BTreeMap<Metric, Option<Spread>>,
}

impl ViewReport {
    pub(super) fn new(name: &str) -> Self {
        Self { name: name.into(), samples: Vec::new(), summary: BTreeMap::new() }
    }

    pub(super) fn summarize(&mut self) {
        self.summary = Metric::ALL
            .into_iter()
            .map(|metric| {
                let values = self.samples.iter().filter_map(|sample| sample.values.get(&metric).copied().flatten());
                (metric, Spread::calculate(values.collect()))
            })
            .collect();
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Report {
    schema: u32,
    label: String,
    idn_version: String,
    started_unix_s: f64,
    card_mode: Option<String>,
    signal_channel: u8,
    settle_s: f64,
    window_s: f64,
    views: Vec<ViewReport>,
}

impl Report {
    pub(super) fn new(
        options: &BenchmarkOptions<'_>,
        started: Duration,
        version: &str,
        card_mode: Option<String>,
        views: Vec<ViewReport>,
    ) -> Self {
        Self {
            schema: SCHEMA,
            label: options.label.into(),
            idn_version: version.into(),
            started_unix_s: started.as_secs_f64(),
            card_mode,
            signal_channel: options.signal_channel,
            settle_s: SETTLE.as_secs_f64(),
            window_s: WINDOW.as_secs_f64(),
            views,
        }
    }
}

pub(super) fn compare_files(a: &Path, b: &Path) -> Result<(), Error> {
    let left = report::read(a, "A").map_err(Error::Report)?;
    let right = report::read(b, "B").map_err(Error::Report)?;
    compare(&left, &right, &mut io::stdout().lock())
}

pub(super) fn compare(a: &Report, b: &Report, out: &mut impl io::Write) -> Result<(), Error> {
    if a.schema != SCHEMA || b.schema != SCHEMA {
        return Err(Error::Report("unsupported public-bench report schema".into()));
    }
    if a.views.iter().map(|view| &view.name).ne(b.views.iter().map(|view| &view.name)) {
        return Err(Error::Report("public-bench reports have different view lists".into()));
    }
    if a.card_mode != b.card_mode {
        let left = a.card_mode.as_deref().unwrap_or("not reported");
        let right = b.card_mode.as_deref().unwrap_or("not reported");
        writeln!(out, "warning: boot card mode differs (A: {left}, B: {right})").map_err(Error::Output)?;
    }
    if a.signal_channel != b.signal_channel || a.settle_s != b.settle_s || a.window_s != b.window_s {
        writeln!(out, "warning: benchmark configuration differs").map_err(Error::Output)?;
    }
    for (left, right) in a.views.iter().zip(&b.views) {
        if settings_differ(left, right) || settings_differ(right, left) {
            writeln!(out, "warning: recorded settings differ for {}", left.name).map_err(Error::Output)?;
        }
    }
    writeln!(out, "A: {} ({})    B: {} ({})", a.label, a.idn_version, b.label, b.idn_version).map_err(Error::Output)?;
    writeln!(
        out,
        "{:17} {:22} {:>29} {:>29} {:>8}",
        "view", "metric", "A median [min, max]", "B median [min, max]", "B/A"
    )
    .map_err(Error::Output)?;
    for (left, right) in a.views.iter().zip(&b.views) {
        write_view(out, left, right).map_err(Error::Output)?;
    }
    Ok(())
}

// Compare each distinct setting snapshot. Repeating an identical snapshot does
// not cause a warning when the benchmarks use different repetition counts.
fn settings_differ(a: &ViewReport, b: &ViewReport) -> bool {
    a.samples.iter().any(|sample| !b.samples.iter().any(|other| sample.settings == other.settings))
}

fn write_view(out: &mut impl io::Write, a: &ViewReport, b: &ViewReport) -> io::Result<()> {
    for metric in Metric::ALL {
        let left = a.summary.get(&metric).and_then(Option::as_ref);
        let right = b.summary.get(&metric).and_then(Option::as_ref);
        let ratio = left
            .zip(right)
            .filter(|(left, _)| left.median != 0.0)
            .map(|(left, right)| format!("{:.3}", right.median / left.median));
        let show = |spread: Option<&Spread>| spread.map_or_else(|| "not reported".to_owned(), Spread::display);
        writeln!(
            out,
            "{:17} {:22} {:>29} {:>29} {:>8}",
            a.name,
            metric.label(),
            show(left),
            show(right),
            ratio.as_deref().unwrap_or("-")
        )?;
    }
    Ok(())
}
