// SPDX-License-Identifier: GPL-2.0-or-later

mod build;
mod cache;
mod process;
mod ssh;

use super::{
    command::Run,
    measure,
    report::{AbReport, Group, Report},
};
use crate::unit_tools::{
    report::PendingReport,
    scpi::{Scpi, TcpScpi, PORT},
    with_restore,
};
use build::Build;
use process::Scratch;
use ssh::{Remote, Ssh};
use std::{collections::BTreeMap, path::Path};

const BENCHMARK: &str = "drawpipe::stage::fixture_bench::raw_batch_stage_benchmark";
const RESTART: &str = r#"printf ':RUN\n' | nc -w 2 "$SCPI_HOST" 5555"#;

pub(super) fn run(root: &Path, options: &Run<'_>) -> Result<(), String> {
    let pending = PendingReport::open(&options.out).map_err(|e| format!("open --out: {e}"))?;
    let reports = session(root, options, None)?;
    pending.write(&reports.result).map_err(|e| format!("write result: {e}"))
}

pub(super) fn ab(root: &Path, options: &Run<'_>, reference_checkout: &Path) -> Result<AbReport, String> {
    let pending = PendingReport::open(&options.out).map_err(|e| format!("open --out: {e}"))?;
    let reports = session(root, options, Some(reference_checkout))?;
    let reference = reports.reference.expect("the A/B session includes a reference checkout");
    let report = AbReport { reference, result: reports.result };
    pending.write(&report).map_err(|e| format!("write A/B result: {e}"))?;
    Ok(report)
}

fn session(root: &Path, options: &Run<'_>, reference_checkout: Option<&Path>) -> Result<Sides<Report>, String> {
    let target = root.join("target");
    let result = Build::prepare(root, &target)?;
    let groups = cache::groups(&options.fixtures, &options.groups)?;
    let scratch = Scratch::new(&target)?;
    let reference = reference_checkout
        .map(|path| Build::prepare(path, &scratch.path().join("reference-target")))
        .transpose()
        .map_err(|error| format!("reference checkout: {error}"))?;
    let mut remote = Ssh::new(options.unit, &scratch);
    let mut scpi = TcpScpi::connect(remote.hostname()?, PORT)?;
    let (core_type, captures) =
        capture(&mut remote, &mut scpi, &scratch, options, Sides { reference, result }, &groups)?;
    captures.try_map(|capture| capture.summarize(options.cpu, &core_type))
}

#[derive(Debug)]
struct Sides<T> {
    reference: Option<T>,
    result: T,
}

impl<T> Sides<T> {
    fn try_map<U>(self, mut map: impl FnMut(T) -> Result<U, String>) -> Result<Sides<U>, String> {
        Ok(Sides { reference: self.reference.map(&mut map).transpose()?, result: map(self.result)? })
    }

    fn round_order(&mut self, round: usize) -> impl Iterator<Item = (&'static str, &mut T)> {
        let paired = self.reference.is_some();
        let mut order = [
            self.reference.as_mut().map(|side| (" reference", side)),
            Some((if paired { " result" } else { "" }, &mut self.result)),
        ];
        if round.is_multiple_of(2) {
            order.reverse();
        }
        order.into_iter().flatten()
    }
}

fn capture(
    remote: &mut impl Remote,
    scpi: &mut impl Scpi,
    scratch: &Scratch,
    options: &Run<'_>,
    builds: Sides<Build>,
    groups: &[String],
) -> Result<(String, Sides<Capture>), String> {
    with_scope_stopped(scpi, options.stop, || {
        let cpu = options.cpu;
        let core_type = remote.run(
            "read CPU identification",
            &format!("cat /sys/devices/system/cpu/cpu{cpu}/regs/identification/midr_el1"),
            15,
            None,
        )?;
        let core_type = core_type.trim();
        if !core_type.strip_prefix("0x").is_some_and(|midr| hex(midr, 16)) {
            return Err("CPU has no kernel-reported MIDR".into());
        }
        let mut captures = builds.try_map(|build| Capture::prepare(remote, scratch, build))?;
        let cached = groups
            .iter()
            .map(|name| cache::fixture(remote, scratch, &options.fixtures.join(name)).map(|path| (name, path)))
            .collect::<Result<Vec<_>, _>>()?;
        for (name, cache::Fixture { path, sha256 }) in cached {
            for round in 1..=options.rounds {
                for (side, capture) in captures.round_order(round) {
                    let operation = format!("group '{name}',{side} round {round}");
                    let output = capture.round(remote, cpu, &path, &operation)?;
                    capture
                        .groups
                        .entry(name.clone())
                        .or_insert_with(|| GroupCapture { sha256: sha256.clone(), rounds: Vec::new() })
                        .rounds
                        .push(output);
                }
            }
        }
        Ok((core_type.into(), captures))
    })
}

fn with_scope_stopped<T>(
    scpi: &mut impl Scpi,
    allow_stop: bool,
    work: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if !scpi.acquiring()? {
        return work();
    }
    if !allow_stop {
        return Err("scope is acquiring; stop it or use --stop".into());
    }
    eprintln!("If interrupted, set SCPI_HOST to the unit's SCPI hostname and run: {RESTART}");
    with_restore(scpi, resume, |scpi| {
        scpi.send(":STOP")?;
        if scpi.acquiring()? {
            return Err("scope did not stop acquiring".into());
        }
        work()
    })
}

fn resume(scpi: &mut impl Scpi) -> Result<(), String> {
    if let Err(first) = restore_run(scpi) {
        return scpi.reconnect().and_then(|()| restore_run(scpi)).map_err(|retry| {
            format!("could not confirm :RUN restoration: {first}; retry: {retry}. Set SCPI_HOST and run: {RESTART}")
        });
    }
    Ok(())
}

fn restore_run(scpi: &mut impl Scpi) -> Result<(), String> {
    scpi.send(":RUN")?;
    scpi.acquiring()?.then_some(()).ok_or_else(|| "scope still reports stopped".into())
}

#[derive(Debug)]
struct GroupCapture {
    sha256: String,
    rounds: Vec<String>,
}

#[derive(Debug)]
struct Capture {
    commit: String,
    runner: String,
    binary_sha256: String,
    groups: BTreeMap<String, GroupCapture>,
}

impl Capture {
    fn prepare(remote: &mut impl Remote, scratch: &Scratch, build: Build) -> Result<Self, String> {
        let (runner, binary_sha256) = cache::binary(remote, scratch, &build.binary)?;
        let listed =
            remote.run("list benchmark test", &format!("{runner} {BENCHMARK} --ignored --exact --list"), 15, None)?;
        if !listed.lines().any(|line| line.strip_suffix(": test") == Some(BENCHMARK)) {
            return Err("built library test executable does not list the benchmark".into());
        }
        Ok(Self { commit: build.commit, runner, binary_sha256, groups: BTreeMap::new() })
    }

    fn round(&self, remote: &mut impl Remote, cpu: u32, path: &str, operation: &str) -> Result<String, String> {
        let runner = &self.runner;
        let script = format!(
            "exec timeout -s KILL 120 env -u OOS_BENCH_STAGE OOS_FIXTURES={path} OOS_BENCH_CPU={cpu} \
             OOS_BENCH_REPEATS=1 {runner} {BENCHMARK} --ignored --nocapture --exact --test-threads=1"
        );
        remote.run(operation, &script, 135, None)
    }

    fn summarize(self, cpu: u32, core_type: &str) -> Result<Report, String> {
        let mut reports = BTreeMap::new();
        for (name, GroupCapture { sha256, rounds }) in self.groups {
            let stages = measure::summarize(rounds).map_err(|e| format!("group '{name}': {e}"))?;
            reports.insert(name, Group::new(sha256, stages));
        }
        Ok(Report::new(self.commit, self.binary_sha256, cpu, core_type.into(), reports))
    }
}

fn hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests;
