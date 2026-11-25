// SPDX-License-Identifier: GPL-2.0-or-later

mod report;
mod settings;
mod state;
#[cfg(test)]
mod tests;

use crate::unit_tools::{
    report::PendingReport,
    scpi::{Scpi, TcpScpi, PORT, STATE},
    Flags,
};
use report::{Metric, Report, Sample, Spread, ViewReport};
use settings::VIEWS;
use state::{read_state, State};
use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
    process::ExitCode,
    thread,
    time::{Duration, Instant, SystemTime},
};

const USAGE: &str = "usage: cargo xtask public-bench --host <addr> --label <label> \
--signal-channel <1-4> [--runs <n>] --out <dir>\n       cargo xtask public-bench compare <a.json> <b.json>";
const SETTLE: Duration = Duration::from_secs(6);
const WINDOW: Duration = Duration::from_secs(12);
const PROBE_INTERVAL: Duration = Duration::from_millis(400);
const OPC: &str = "*OPC?";

#[derive(Debug)]
enum Error {
    Scpi(String),
    InvalidReply(&'static str),
    Report(String),
    Output(io::Error),
    Measurement(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scpi(error) | Self::Report(error) => f.write_str(error),
            Self::InvalidReply(query) => write!(f, "invalid reply to {query}"),
            Self::Output(error) => write!(f, "write benchmark report: {error}"),
            Self::Measurement(error) => f.write_str(error),
        }
    }
}

#[derive(Debug)]
struct BenchmarkOptions<'a> {
    host: &'a str,
    label: &'a str,
    signal_channel: u8,
    runs: usize,
    out: PathBuf,
}

impl<'a> BenchmarkOptions<'a> {
    fn parse(args: &[&'a str]) -> Result<Self, String> {
        let flags = Flags::parse(args, &["--host", "--label", "--signal-channel", "--runs", "--out"], None)?;
        let host = flags.required("--host")?;
        let label = flags.required("--label")?;
        if !label.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            || label.eq_ignore_ascii_case(host.trim_end_matches('.'))
        {
            return Err(
                "--label must be a non-address label using only letters, digits, hyphens and underscores".into()
            );
        }
        let signal_channel = flags
            .required("--signal-channel")?
            .parse::<u8>()
            .ok()
            .filter(|n| (1..=4).contains(n))
            .ok_or("--signal-channel must be 1 through 4")?;
        let runs = flags
            .get("--runs")
            .unwrap_or("3")
            .parse::<usize>()
            .ok()
            .filter(|&n| n > 0)
            .ok_or("--runs must be a positive integer")?;
        Ok(Self { host, label, signal_channel, runs, out: flags.required("--out")?.into() })
    }

    fn measure(&self) -> Result<(), Error> {
        fs::create_dir_all(&self.out).map_err(Error::Output)?;
        let destination = self.out.join(format!("public-{}.json", self.label));
        let pending = PendingReport::open(&destination).map_err(Error::Output)?;
        let started = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|_| Error::Measurement("host time precedes the Unix epoch"))?;
        let mut scope = TcpScpi::connect(self.host.into(), PORT).map_err(Error::Scpi)?;
        let report = collect(&mut scope, &HostClock(Instant::now()), self, started)?;
        pending.write(&report).map_err(Error::Output)
    }
}

pub(super) fn run(args: &[&str]) -> ExitCode {
    let result = match args {
        ["--help" | "-h"] | ["compare", "--help" | "-h"] => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        ["compare", a, b] => report::compare_files(Path::new(a), Path::new(b)),
        _ => match BenchmarkOptions::parse(args) {
            Ok(options) => options.measure(),
            Err(error) => {
                eprintln!("{error}\n{USAGE}");
                return ExitCode::from(2);
            }
        },
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("public-bench: {error}");
            ExitCode::FAILURE
        }
    }
}

trait Clock {
    fn now(&self) -> Duration;
    fn wait(&self, duration: Duration);
}

#[derive(Debug)]
struct HostClock(Instant);

impl Clock for HostClock {
    fn now(&self) -> Duration {
        self.0.elapsed()
    }

    fn wait(&self, duration: Duration) {
        thread::sleep(duration);
    }
}

fn collect(
    scope: &mut impl Scpi,
    clock: &impl Clock,
    options: &BenchmarkOptions<'_>,
    started: Duration,
) -> Result<Report, Error> {
    let identity = scope.query("*IDN?").map_err(Error::Scpi)?;
    let version = identity
        .split(',')
        .nth(3)
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .ok_or(Error::InvalidReply("*IDN?"))?;
    let state = State::parse(&scope.query(STATE).map_err(Error::Scpi)?)?;
    let mut views: Vec<ViewReport> = VIEWS.iter().map(|view| ViewReport::new(view.name)).collect();
    scope.send("*CLS").map_err(Error::Scpi)?;
    for run in 1..=options.runs {
        for (view, report) in VIEWS.iter().zip(&mut views) {
            eprintln!("public-bench: run {run}/{} {}", options.runs, view.name);
            let mut settings = settings::apply(scope, view, options.signal_channel)?;
            clock.wait(SETTLE);
            settings::read_effective_depth(scope, &mut settings)?;
            let (elapsed, values) = measure_window(scope, clock)?;
            report.samples.push(Sample { settings, elapsed_s: elapsed.as_secs_f64(), values });
        }
    }
    for view in &mut views {
        view.summarize();
    }
    Ok(Report::new(options, started, version, state.card_mode, views))
}

fn measure_window(scope: &mut impl Scpi, clock: &impl Clock) -> Result<(Duration, report::Values), Error> {
    let (start, before) = read_state(scope, clock)?;
    let end = start + WINDOW;
    let mut latencies = Vec::new();
    while clock.now() < end {
        let reply = scope.query_timed(OPC, || clock.now()).map_err(Error::Scpi)?;
        if reply.line.trim() != "1" {
            return Err(Error::InvalidReply(OPC));
        }
        latencies.push((reply.after_line - reply.before_write).as_secs_f64() * 1_000.0);
        clock.wait(PROBE_INTERVAL.min(end.saturating_sub(clock.now())));
    }
    let (stop, after) = read_state(scope, clock)?;
    let duration = stop.saturating_sub(start);
    if duration.is_zero() {
        return Err(Error::Measurement("SCPI counter snapshots have no elapsed time"));
    }
    let mut values = after.values_since(&before, duration);
    let latency = Spread::calculate(latencies);
    values.insert(Metric::OpcHostMedianMs, latency.as_ref().map(|spread| spread.median));
    values.insert(Metric::OpcHostMaxMs, latency.as_ref().map(|spread| spread.max));
    Ok((duration, values))
}
