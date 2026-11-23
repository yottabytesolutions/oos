// SPDX-License-Identifier: GPL-2.0-or-later

mod command;
mod measure;
mod report;
mod scope;

use self::command::Options;
use crate::unit_tools::report::read;
use std::{path::Path, process::ExitCode};

pub(super) fn run(root: &Path, args: &[&str]) -> ExitCode {
    match Options::parse(args) {
        Ok(Options::Help) => {
            println!("{}\nRun defaults to {} rounds on CPU 2.", command::USAGE, command::DEFAULT_ROUNDS);
            ExitCode::SUCCESS
        }
        Ok(Options::Run(options)) => finish(scope::run(root, &options).map(|()| ExitCode::SUCCESS)),
        Ok(Options::Compare(options)) => finish(compare_files(&options)),
        Ok(Options::Ab { run, reference_checkout, tolerance }) => finish(
            scope::ab(root, &run, &reference_checkout)
                .and_then(|reports| compare(&reports.reference, &reports.result, tolerance)),
        ),
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

fn compare_files(options: &command::Compare) -> Result<ExitCode, String> {
    compare(&read(&options.reference, "reference")?, &read(&options.result, "result")?, options.tolerance)
}

fn compare(reference: &report::Report, result: &report::Report, tolerance: f64) -> Result<ExitCode, String> {
    let rows = report::compare(reference, result, tolerance)?;
    let placement = report::Placement::summarize(&rows, tolerance);
    print!("{}", report::table(&rows));
    println!("{}", placement.summary());
    Ok(if rows.iter().any(report::Row::insufficient) {
        ExitCode::from(2)
    } else if placement.regression() || rows.iter().any(report::Row::regression) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
