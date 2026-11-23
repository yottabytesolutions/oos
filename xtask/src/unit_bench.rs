// SPDX-License-Identifier: GPL-2.0-or-later

mod command;
mod measure;
mod report;
mod scope;

use self::command::Options;
use self::report::Report;
use std::{fs, path::Path, process::ExitCode};

pub(super) fn run(root: &Path, args: &[&str]) -> ExitCode {
    match Options::parse(args) {
        Ok(Options::Help) => {
            println!("{}\nRun defaults to {} rounds on CPU 2.", command::USAGE, command::DEFAULT_ROUNDS);
            ExitCode::SUCCESS
        }
        Ok(Options::Run(options)) => finish(scope::run(root, &options).map(|()| ExitCode::SUCCESS)),
        Ok(Options::Compare(options)) => finish(compare(&options.reference, &options.result, options.tolerance)),
        Err(error) => {
            eprintln!("unit-bench: {error}\n{}", command::USAGE);
            ExitCode::from(2)
        }
    }
}

fn finish(result: Result<ExitCode, String>) -> ExitCode {
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("unit-bench: {error}");
            ExitCode::from(2)
        }
    }
}

fn read(path: &Path, label: &str) -> Result<Report, String> {
    let data = fs::read(path).map_err(|e| format!("read {label} report: {e}"))?;
    serde_json::from_slice(&data).map_err(|e| format!("invalid {label} report: {e}"))
}

fn compare(reference: &Path, result: &Path, tolerance: f64) -> Result<ExitCode, String> {
    let reference = read(reference, "reference")?;
    let rows = report::compare(&reference, &read(result, "result")?, tolerance)?;
    print!("{}", report::table(&rows));
    Ok(if rows.iter().any(report::Row::insufficient) {
        ExitCode::from(2)
    } else if rows.iter().any(report::Row::regression) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
