// SPDX-License-Identifier: GPL-2.0-or-later

mod cache;
mod output;
mod process;
mod scpi;
mod ssh;

use super::{
    command::Run,
    measure,
    report::{Group, Report},
};
use output::PendingReport;
use process::Scratch;
use scpi::{Scpi, TcpScpi};
use ssh::{Remote, Ssh};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

const BENCHMARK: &str = "drawpipe::stage::fixture_bench::raw_batch_stage_benchmark";
const RESTART: &str = r#"printf ':RUN\n' | nc -w 2 "$SCPI_HOST" 5555"#;

pub(super) fn run(root: &Path, options: &Run<'_>) -> Result<(), String> {
    let pending = PendingReport::open(&options.out).map_err(|e| format!("open --out: {e}"))?;
    let binary = build(root)?;
    let commit = commit(root)?;
    let groups = cache::groups(&options.fixtures, &options.groups)?;
    let scratch = Scratch::new(&root.join("target"))?;
    let mut remote = Ssh::new(options.unit, &scratch);
    let mut scpi = TcpScpi::connect(remote.hostname()?)?;
    let capture = capture(&mut remote, &mut scpi, &scratch, options, &binary, &groups)?;
    let report = capture.summarize(options, commit)?;
    pending.write(&report).map_err(|e| format!("write result: {e}"))
}

fn build(root: &Path) -> Result<PathBuf, String> {
    let toolchain = toolchain(root)?;
    let output = Command::new("cargo")
        .arg(format!("+{toolchain}"))
        .args([
            "test",
            "--locked",
            "-p",
            "oos-ui",
            "--lib",
            "--release",
            "--target",
            crate::repo::TARGET,
            "--features",
            "private-picture-counts",
            "--no-run",
            "--message-format=json-render-diagnostics",
        ])
        .current_dir(root)
        .env("CARGO_TARGET_DIR", root.join("target"))
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "0")
        .env("CARGO_PROFILE_TEST_DEBUG", "0")
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("start benchmark build: {e}"))?;
    if !output.status.success() {
        return Err(format!("benchmark build failed with {}", output.status));
    }
    executable(&String::from_utf8_lossy(&output.stdout))
}

fn toolchain(root: &Path) -> Result<String, String> {
    let pins = std::fs::read_to_string(root.join("rust-toolchain.toml")).map_err(|_| "read rust-toolchain.toml")?;
    let channel = pins
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .skip_while(|line| *line != "[toolchain]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == "channel")
        .and_then(|(_, value)| value.trim().strip_prefix('"')?.strip_suffix('"'))
        .ok_or("rust-toolchain.toml must set a quoted toolchain channel")?;
    let parts: Vec<_> = channel.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit())) {
        return Err("rust-toolchain.toml must pin an exact Rust version".into());
    }
    Ok(channel.to_owned())
}

fn executable(cargo_json: &str) -> Result<PathBuf, String> {
    let mut artifacts = cargo_json
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|item| {
            item["reason"] == "compiler-artifact"
                && item["target"]["name"] == "oos_ui"
                && item["target"]["kind"].as_array().is_some_and(|kinds| kinds.iter().any(|kind| kind == "lib"))
                && item["profile"]["test"] == true
        })
        .filter_map(|item| item["executable"].as_str().map(PathBuf::from));
    match (artifacts.next(), artifacts.next()) {
        (Some(path), None) => Ok(path),
        _ => Err("Cargo did not identify exactly one oos-ui library test executable".into()),
    }
}

fn commit(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%H"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("read checkout commit: {e}"))?;
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || !hex(&commit, 40) {
        return Err("could not read checkout commit".into());
    }
    Ok(commit)
}

fn capture(
    remote: &mut impl Remote,
    scpi: &mut impl Scpi,
    scratch: &Scratch,
    options: &Run<'_>,
    binary: &Path,
    groups: &[String],
) -> Result<Capture, String> {
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
        let (runner, binary_sha256) = cache::binary(remote, scratch, binary)?;
        let listed =
            remote.run("list benchmark test", &format!("{runner} {BENCHMARK} --ignored --exact --list"), 15, None)?;
        if !listed.lines().any(|line| line.strip_suffix(": test") == Some(BENCHMARK)) {
            return Err("built library test executable does not list the benchmark".into());
        }
        let cached = groups
            .iter()
            .map(|name| cache::fixture(remote, scratch, &options.fixtures.join(name)).map(|path| (name, path)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut outputs = BTreeMap::new();
        for (name, cache::Fixture { path, sha256 }) in cached {
            let script = format!(
                "exec timeout -s KILL 120 env -u OOS_BENCH_STAGE OOS_FIXTURES={path} OOS_BENCH_CPU={cpu} \
                 OOS_BENCH_REPEATS=1 {runner} {BENCHMARK} --ignored --nocapture --exact --test-threads=1"
            );
            let rounds = (1..=options.rounds)
                .map(|round| remote.run(&format!("group '{name}', round {round}"), &script, 135, None))
                .collect::<Result<Vec<_>, _>>()?;
            outputs.insert(name.clone(), GroupCapture { sha256, rounds });
        }
        Ok(Capture { binary_sha256, core_type: core_type.into(), groups: outputs })
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
    let mut restoration = Ok(());
    let result = {
        let mut guard = RestoreRun { scpi, restoration: &mut restoration };
        guard.stop().and_then(|()| work())
    };
    match (result, restoration) {
        (Err(error), Err(restore)) => Err(format!("{error}; {restore}")),
        (Ok(_), Err(restore)) => Err(restore),
        (result, Ok(())) => result,
    }
}

struct RestoreRun<'a, S: Scpi> {
    scpi: &'a mut S,
    restoration: &'a mut Result<(), String>,
}

impl<S: Scpi> RestoreRun<'_, S> {
    fn stop(&mut self) -> Result<(), String> {
        self.scpi.send(":STOP")?;
        if self.scpi.acquiring()? {
            return Err("scope did not stop acquiring".into());
        }
        Ok(())
    }
}

impl<S: Scpi> Drop for RestoreRun<'_, S> {
    fn drop(&mut self) {
        if let Err(first) = restore_run(self.scpi) {
            *self.restoration = self.scpi.reconnect().and_then(|()| restore_run(self.scpi)).map_err(|retry| {
                format!("could not confirm :RUN restoration: {first}; retry: {retry}. Set SCPI_HOST and run: {RESTART}")
            });
        }
    }
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
    binary_sha256: String,
    core_type: String,
    groups: BTreeMap<String, GroupCapture>,
}

impl Capture {
    fn summarize(self, options: &Run<'_>, commit: String) -> Result<Report, String> {
        let mut reports = BTreeMap::new();
        for (name, GroupCapture { sha256, rounds }) in self.groups {
            let stages = measure::summarize(rounds).map_err(|e| format!("group '{name}': {e}"))?;
            reports.insert(name, Group::new(sha256, stages));
        }
        Ok(Report::new(commit, self.binary_sha256, options.cpu, self.core_type, reports))
    }
}

fn hex(text: &str, len: usize) -> bool {
    text.len() == len && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests;
