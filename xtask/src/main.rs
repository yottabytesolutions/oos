//! The repository gate (docs/development.md).
//!
//!   cargo xtask check [--fast] [--no-coverage] [--list] [--image]   the gate; --fast includes syntax, architecture and agent rules
//!   cargo xtask ratchet [--coverage] [--move] [--init]   write the ratchet files back after an improvement

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
mod moves;
mod ratchet;
mod rebaseline;
mod repo;
mod report;
mod run;
mod size;
mod sources;
mod steps;
mod store;
mod target;
#[cfg(test)]
mod testrepo;
mod testrun;
mod tighten;
mod workspace;

use std::process::ExitCode;

const USAGE: &str =
    "usage: cargo xtask doctor [--gpu] [--sim] [--image] | cargo xtask check [--fast] [--no-coverage] [--list] [--image] | cargo xtask ratchet [--coverage] [--move] [--init]";

/// The command line: a command and its flags.
fn parse(args: &[String]) -> Result<(&str, Vec<&str>), String> {
    let (cmd, flags) = args.split_first().ok_or(USAGE)?;
    let flags: Vec<&str> = flags.iter().map(String::as_str).collect();
    let allowed: &[&str] = match cmd.as_str() {
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
    }
}
