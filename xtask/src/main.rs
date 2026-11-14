//! The repository gate (docs/development.md).
//!
//!   cargo xtask check [--fast] [--no-coverage]   the gate; --fast: formatting, size limits, architecture
//!   cargo xtask ratchet [--init] [--coverage]    write the ratchet files back after an improvement

mod arch;
mod base;
mod check;
mod clippy;
mod coverage;
mod fmt;
mod ratchet;
mod repo;
mod report;
mod run;
mod size;
mod sources;
mod steps;
mod target;
mod testrun;
mod tighten;
mod workspace;

use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask check [--fast] [--no-coverage] | cargo xtask ratchet [--init] [--coverage]";

/// The command line: a command and its flags.
fn parse(args: &[String]) -> Result<(&str, Vec<&str>), String> {
    let (cmd, flags) = args.split_first().ok_or(USAGE)?;
    let flags: Vec<&str> = flags.iter().map(String::as_str).collect();
    let allowed: &[&str] = match cmd.as_str() {
        "check" => &["--fast", "--no-coverage"],
        "ratchet" => &["--init", "--coverage"],
        _ => return Err(USAGE.into()),
    };
    match flags.iter().find(|f| !allowed.contains(f)) {
        Some(f) => Err(format!("unknown flag {}\n{}", f, USAGE)),
        None => Ok((cmd.as_str(), flags)),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, flags) = match parse(&args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(2);
        }
    };
    let root = repo::root();
    let ok = match cmd {
        "check" => {
            let o = check::Options { fast: flags.contains(&"--fast"), coverage: !flags.contains(&"--no-coverage") };
            check::run(&root, o)
        }
        _ => tighten::run(&root, flags.contains(&"--init"), flags.contains(&"--coverage"))
            .map_err(|e| eprintln!("{}", e))
            .is_ok(),
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
mod cli {
    use super::*;

    fn a(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn parses_commands_and_refuses_unknown_flags() {
        assert_eq!(parse(&a(&["check", "--fast"])).unwrap(), ("check", vec!["--fast"]));
        assert_eq!(parse(&a(&["ratchet"])).unwrap(), ("ratchet", vec![]));
        assert!(parse(&a(&["check", "--init"])).is_err());
        assert!(parse(&a(&["build"])).is_err() && parse(&[]).is_err());
    }
}
