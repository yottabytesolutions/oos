// SPDX-License-Identifier: GPL-2.0-or-later

mod interrupt;
mod report;
mod restore;
mod settings;
mod state;
#[cfg(test)]
mod tests;
mod trigger;

use crate::unit_tools::{
    report::PendingReport,
    scpi::{Scpi, TcpScpi, PORT, STATE},
    Flags,
};
use interrupt::{HostClock, InterruptibleScpi};
use report::{Metric, Report, Sample, Spread, ViewReport};
use restore::Snapshot;
use settings::VIEWS;
use state::{read_state, State};
use std::{
    fmt, fs, io,
    io::Write as _,
    path::{Path, PathBuf},
    process::ExitCode,
    time::{Duration, SystemTime},
};
use trigger::{Level, Trigger};

const USAGE: &str = "usage: cargo xtask public-bench --host <addr> --label <label> \
--signal-channel <1-4> [--trigger-level <volts|auto>] [--runs <n>] --out <dir>\n       cargo xtask public-bench compare <a.json> <b.json>";
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
    Setting(String),
    Snapshot(&'static str),
    TriggerLevel,
    ZeroRate,
    Restore { failed: Vec<&'static str>, run: Option<Box<Self>> },
    Interrupted(u8),
    InterruptControl(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scpi(error) | Self::Report(error) => f.write_str(error),
            Self::InvalidReply(query) => write!(f, "invalid reply to {query}"),
            Self::Output(error) => write!(f, "write benchmark output: {error}"),
            Self::Measurement(error) => f.write_str(error),
            Self::Setting(header) => write!(f, "could not verify {header} for the trigger check"),
            Self::Snapshot(key) => write!(f, "could not read a restorable value for {key}"),
            Self::TriggerLevel => f.write_str(
                "could not measure distinct signal minimum and maximum for auto trigger level. Use --trigger-level <volts>",
            ),
            Self::ZeroRate => f.write_str("one or more views had no triggers"),
            Self::Restore { failed, run } => {
                if let Some(error) = run {
                    write!(f, "{error}. ")?;
                }
                write!(f, "could not restore or verify settings: {}", failed.join(", "))
            }
            Self::Interrupted(_) => f.write_str("benchmark interrupted"),
            Self::InterruptControl(error) => write!(f, "monitor benchmark termination signals: {error}"),
        }
    }
}

impl Error {
    fn exit_code(&self) -> ExitCode {
        match self {
            Self::Interrupted(code) => ExitCode::from(*code),
            Self::Restore { run: Some(error), .. } => error.exit_code(),
            _ => ExitCode::FAILURE,
        }
    }
}

fn with_restoration<T>(result: Result<T, Error>, failed: Vec<&'static str>) -> Result<T, Error> {
    if failed.is_empty() {
        result
    } else {
        Err(Error::Restore { failed, run: result.err().map(Box::new) })
    }
}

#[derive(Debug)]
struct BenchmarkOptions<'a> {
    host: &'a str,
    label: &'a str,
    signal_channel: u8,
    trigger_level: Level,
    runs: usize,
    out: PathBuf,
}

impl<'a> BenchmarkOptions<'a> {
    fn parse(args: &[&'a str]) -> Result<Self, String> {
        let flags =
            Flags::parse(args, &["--host", "--label", "--signal-channel", "--trigger-level", "--runs", "--out"], None)?;
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
        let trigger_level = match flags.get("--trigger-level").unwrap_or("auto") {
            "auto" => Level::Auto,
            volts => Level::Explicit(
                settings::finite(volts)
                    .filter(|value| (*value as f32).is_finite())
                    .ok_or("--trigger-level must be a finite voltage or auto")?,
            ),
        };
        Ok(Self { host, label, signal_channel, trigger_level, runs, out: flags.required("--out")?.into() })
    }

    fn measure(&self) -> Result<(), Error> {
        let clock = HostClock::start()?;
        fs::create_dir_all(&self.out).map_err(Error::Output)?;
        let destination = self.out.join(format!("public-{}.json", self.label));
        let pending = PendingReport::open(&destination).map_err(Error::Output)?;
        let started = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|_| Error::Measurement("host time precedes the Unix epoch"))?;
        clock.check()?;
        let connection = TcpScpi::connect(self.host.into(), PORT).map_err(Error::Scpi);
        let mut scope = clock.check().and(connection)?;
        let mut out = io::stdout().lock();
        let (report, failed) = collect(&mut scope, &clock, self, started, &mut out)?;
        let result = pending
            .write(&report)
            .map_err(Error::Output)
            .and_then(|()| report.write_summary(&mut out))
            .and_then(|()| report.check_rates());
        with_restoration(clock.check().and(result), failed)
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
        Err(Error::ZeroRate) => ExitCode::FAILURE,
        Err(error) => {
            let _ = writeln!(io::stderr(), "public-bench: {error}");
            error.exit_code()
        }
    }
}

trait Clock {
    fn now(&self) -> Duration;
    fn wait(&self, duration: Duration) -> Result<(), Error>;
    fn check(&self) -> Result<(), Error> {
        Ok(())
    }
}

fn collect(
    scope: &mut impl Scpi,
    clock: &impl Clock,
    options: &BenchmarkOptions<'_>,
    started: Duration,
    out: &mut impl io::Write,
) -> Result<(Report, Vec<&'static str>), Error> {
    let setup = read_setup(&mut InterruptibleScpi::new(scope, clock));
    let (version, state, snapshot) = clock.check().and(setup)?;
    let result = collect_views(&mut InterruptibleScpi::new(scope, clock), clock, options, &snapshot, out);
    let failed = snapshot.restore(scope);
    match result {
        Ok((trigger, views)) => Ok((Report::new(options, started, &version, state.card_mode, views, trigger), failed)),
        Err(error) => with_restoration(clock.check().and(Err(error)), failed),
    }
}

fn read_setup(scope: &mut impl Scpi) -> Result<(String, State, Snapshot), Error> {
    let identity = scope.query("*IDN?").map_err(Error::Scpi)?;
    let version = identity
        .split(',')
        .nth(3)
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .ok_or(Error::InvalidReply("*IDN?"))?;
    scope.send("*CLS").map_err(Error::Scpi)?;
    let initial_state = scope.query(STATE).map_err(Error::Scpi)?;
    let state = State::parse(&initial_state)?;
    let snapshot = Snapshot::read(scope, &initial_state)?;
    Ok((version.to_owned(), state, snapshot))
}

fn collect_views(
    scope: &mut impl Scpi,
    clock: &impl Clock,
    options: &BenchmarkOptions<'_>,
    snapshot: &Snapshot,
    out: &mut impl io::Write,
) -> Result<(Trigger, Vec<ViewReport>), Error> {
    let signal = options.signal_channel;
    let trigger = Trigger::resolve(scope, clock, signal, options.trigger_level)?;
    clock.check()?;
    settings::prepare_trigger(scope, signal, Some(trigger.volts()))?;
    if !trigger::advances_while_settling(scope, clock)? {
        return Err(Error::Measurement(
            "no triggers before the first view: acquired counter did not advance in Normal sweep during the 6 s settle",
        ));
    }
    let mut views: Vec<ViewReport> = VIEWS.iter().map(|view| ViewReport::new(view.name)).collect();
    for run in 1..=options.runs {
        for (view, report) in VIEWS.iter().zip(&mut views) {
            clock.check()?;
            let mut settings = settings::apply(scope, view, signal, trigger.volts(), snapshot.has_auto_depth())?;
            clock.wait(SETTLE)?;
            settings::read_effective_depth(scope, &mut settings)?;
            let (elapsed, values) = measure_window(scope, clock)?;
            report.samples.push(Sample { settings, elapsed_s: elapsed.as_secs_f64(), values });
            report.summarize();
            report.write_progress(out, run, options.runs)?;
        }
    }
    clock.check()?;
    Ok((trigger, views))
}

fn measure_window(scope: &mut impl Scpi, clock: &impl Clock) -> Result<(Duration, report::Values), Error> {
    let (start, before) = read_state(scope, clock)?;
    let end = start + WINDOW;
    let mut latencies = Vec::new();
    while clock.now() < end {
        clock.check()?;
        let reply = scope.query_timed(OPC, || clock.now()).map_err(Error::Scpi)?;
        if reply.line.trim() != "1" {
            return Err(Error::InvalidReply(OPC));
        }
        latencies.push((reply.after_line - reply.before_write).as_secs_f64() * 1_000.0);
        clock.wait(PROBE_INTERVAL.min(end.saturating_sub(clock.now())))?;
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
