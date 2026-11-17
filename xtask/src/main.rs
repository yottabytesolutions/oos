// SPDX-License-Identifier: GPL-2.0-or-later
//! The repository gate (docs/development.md).
//!
//!   cargo xtask check [--fast] [--no-coverage] [--list] [--image]   the gate; --fast includes syntax, architecture and agent rules
//!   cargo xtask step <id>...   only the named inventory steps, in inventory order (CI splits the gate into jobs)
//!   cargo xtask ratchet [--coverage] [--move] [--init]   write the ratchet files back after an improvement
//!   cargo xtask loc [--json] [--compare <git-ref>]   production code lines per crate (ADR 0035)

mod agent_rules;
mod arch;
mod attrs;
mod base;
mod check;
mod clippy;
mod coverage;
mod doctor;
mod fmt;
mod inventory;
mod links;
mod loc;
mod moves;
mod ratchet;
mod repo;
mod report;
mod run;
mod size;
mod sources;
mod steps;
mod store;
mod target;
mod test_time;
#[cfg(test)]
mod testrepo;
mod testrun;
mod tighten;
mod workspace;

use std::process::ExitCode;

const USAGE: &str =
    "usage: cargo xtask doctor [--gpu] [--sim] [--image] | cargo xtask check [--fast] [--no-coverage] [--list] [--image] | cargo xtask step <id>... | cargo xtask ratchet [--coverage] [--move] [--init] | cargo xtask loc [--json] [--compare <git-ref>]";

/// The command line: a command and its flags.
fn parse(args: &[String]) -> Result<(&str, Vec<&str>), String> {
    let (cmd, flags) = args.split_first().ok_or(USAGE)?;
    let flags: Vec<&str> = flags.iter().map(String::as_str).collect();
    let allowed: &[&str] = match cmd.as_str() {
        "step" if flags.is_empty() => return Err(USAGE.into()),
        "step" => return check::known(&flags).map(|()| (cmd.as_str(), flags)),
        "loc" => return loc::Options::parse(&flags).map(|_| (cmd.as_str(), flags)),
        "doctor" => &["--gpu", "--sim", "--image"],
        "check" => &["--fast", "--no-coverage", "--list", "--image"],
        "ratchet" => &["--init", "--coverage", "--move"],
        _ => return Err(USAGE.into()),
    };
    if cmd == "check" && flags.contains(&"--image") && (flags.contains(&"--fast") || flags.contains(&"--no-coverage")) {
        return Err("--image requires the full host gate".into());
    }
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
        "doctor" => doctor::run(&root, &flags),
        "check" => {
            let o = check::Options { fast: flags.contains(&"--fast"), coverage: !flags.contains(&"--no-coverage") };
            check::run(&root, o, flags.contains(&"--image"), flags.contains(&"--list"))
        }
        "step" => check::steps(&root, &flags),
        "loc" => loc::run(&root, &flags),
        _ => tighten::run(
            &root,
            tighten::Options {
                init: flags.contains(&"--init"),
                coverage: flags.contains(&"--coverage"),
                moves: flags.contains(&"--move"),
            },
        )
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
        assert_eq!(parse(&a(&["step", "clippy", "fmt"])).unwrap(), ("step", vec!["clippy", "fmt"]));
        assert!(parse(&a(&["step"])).is_err());
        assert!(parse(&a(&["step", "nope"])).is_err());
        assert_eq!(parse(&a(&["loc", "--compare", "v0.2.0"])).unwrap(), ("loc", vec!["--compare", "v0.2.0"]));
        assert!(parse(&a(&["loc", "--compare"])).is_err() && parse(&a(&["loc", "--fast"])).is_err());
    }
}
