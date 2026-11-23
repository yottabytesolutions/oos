// SPDX-License-Identifier: GPL-2.0-or-later

mod command;
mod compare;
mod plan;
mod report;
mod setup;
mod state;
mod window;

use self::{
    command::{Options, Run, Timing},
    plan::{Group, Plan, Setup},
    report::{GroupReport, Readback, Report},
    setup::Snapshot,
    window::{Clock, Failure, HostClock, WindowSample, COUNTER_RESET},
};
use crate::unit_tools::{
    report::PendingReport,
    scpi::{Scpi, TcpScpi, STATE},
    with_restore,
};
use std::{process::ExitCode, time::Instant};

pub(super) fn run(args: &[&str]) -> ExitCode {
    let result = match Options::parse(args) {
        Ok(Options::Help) => {
            println!("{}", command::USAGE);
            return ExitCode::SUCCESS;
        }
        Ok(Options::Run(options)) => run_live(&options),
        Ok(Options::Compare { reference, result, floor }) => compare::run(&reference, &result, floor),
        Err(error) => Err(format!("{error}\n{}", command::USAGE)),
    };
    result.unwrap_or_else(|error| {
        eprintln!("unit-matrix: {error}");
        ExitCode::from(2)
    })
}

fn run_live(options: &Run<'_>) -> Result<ExitCode, String> {
    let mut plan = Plan::load()?;
    if let Some(path) = &options.bench {
        plan.replace_bench(path)?;
    }
    let groups = plan.select(&options.groups)?;
    let bench_sha256 = plan.bench_sha256()?;
    let pending = PendingReport::open(&options.out).map_err(|e| format!("open --out: {e}"))?;
    let mut scpi = TcpScpi::connect_with_pace(options.host.into(), options.port, options.pace)?;
    let report = capture(&mut scpi, &mut HostClock(Instant::now()), &plan, &groups, options.timing, bench_sha256)?;
    pending.write(&report).map_err(|e| format!("write result: {e}"))?;
    if let Some(error) = report.error {
        return Err(error);
    }
    Ok(if report.groups.values().any(|group| group.error.is_some()) { ExitCode::from(2) } else { ExitCode::SUCCESS })
}

fn capture(
    scpi: &mut impl Scpi,
    clock: &mut impl Clock,
    plan: &Plan,
    groups: &[&Group],
    timing: Timing,
    bench_sha256: String,
) -> Result<Report, String> {
    let build = build(&scpi.query("*IDN?")?)?;
    let snapshot = Snapshot::read(scpi, plan.keys())?;
    snapshot.check_replayable(&plan.default_target())?;
    eprintln!("{}", snapshot.recovery_hint());
    let mut report = Report {
        build,
        base_setup: plan.base_setup.clone(),
        bench_sha256,
        window_s: timing.window.as_secs_f64(),
        skipped_setup_keys: snapshot.skipped_keys.clone(),
        groups: Default::default(),
        error: None,
    };
    let work = with_restore(
        scpi,
        |scpi| snapshot.restore(scpi),
        |scpi| {
            for group in groups {
                let row = GroupReport {
                    overrides: group.overrides.clone(),
                    expected_windows: timing.windows.unwrap_or(group.windows),
                    ..GroupReport::default()
                };
                let row = report.groups.entry(group.id.clone()).or_insert(row);
                let failure = measure(scpi, clock, timing, group, &plan.target(group), &snapshot, row).err();
                row.summarize()?;
                if let Some(failure) = failure {
                    let unhealthy = matches!(failure, Failure::Unhealthy);
                    let message = failure.message();
                    eprintln!("{}: {message}", group.id);
                    row.error = Some(message.clone());
                    if unhealthy {
                        return Err(message);
                    }
                }
            }
            Ok(())
        },
    );
    report.error = work.err();
    Ok(report)
}

fn measure(
    scpi: &mut impl Scpi,
    clock: &mut impl Clock,
    timing: Timing,
    group: &Group,
    target: &Setup,
    snapshot: &Snapshot,
    row: &mut GroupReport,
) -> Result<(), Failure> {
    if !state::State::parse(&scpi.query(STATE)?).healthy {
        return Err(Failure::Unhealthy);
    }
    snapshot.apply(scpi, target)?;
    setup::set_run(scpi, true)?;
    clock.wait(timing.warmup);
    row.readback = Some(readback(scpi)?);
    for index in 1..=row.expected_windows {
        let window = window::measure(scpi, clock, timing.window)?;
        let WindowSample::Valid(sample) = &window.sample else {
            row.windows.push(window);
            return Err(COUNTER_RESET.to_owned().into());
        };
        if timing.sustained {
            let rates = &sample.rates;
            println!(
                "{} {index:>3}  t={:.1}s  fps={:.2}–{:.2}  read/s={:.1}  first/s={:.1}  shown/s={:.1}  late flips/min={:.2}",
                group.id,
                window.start_s,
                rates.fresh_fps[0],
                rates.fresh_fps[1],
                rates.read_per_s,
                rates.first_records_per_s,
                rates.shown_records_per_s,
                window::per_minute(rates.late_flips as f64, window.host_s)
            );
        }
        row.windows.push(window);
    }
    Ok(())
}

fn readback(scpi: &mut impl Scpi) -> Result<Readback, String> {
    let mut query = |command: &str| {
        scpi.query(command).and_then(|reply| {
            if reply.is_empty() {
                Err(format!("empty read-back for {command}"))
            } else {
                Ok(reply)
            }
        })
    };
    Ok(Readback {
        timebase: query(":TIM:MAIN:SCAL?")?,
        memory_depth: query(":ACQ:MDEP?")?,
        sample_rate: query(":ACQ:SRAT?")?,
        acquisition_type: query(":ACQ:TYPE?")?,
    })
}

fn build(identity: &str) -> Result<String, String> {
    let mut fields = identity.splitn(4, ',');
    match (fields.next(), fields.nth(2)) {
        (Some("OOS"), Some(build)) if build.starts_with("oos-ui ") => Ok(build.into()),
        _ => Err("*IDN? did not identify an OOS build".into()),
    }
}

#[cfg(test)]
mod tests;
