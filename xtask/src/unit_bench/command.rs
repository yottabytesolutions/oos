// SPDX-License-Identifier: GPL-2.0-or-later

use std::{collections::BTreeMap, path::PathBuf};

pub(super) const DEFAULT_ROUNDS: usize = super::report::MIN_ROUNDS;

pub(super) const USAGE: &str = "\
usage: cargo xtask unit-bench run --unit <ssh destination> --fixtures <dir> --out <file.json>
           [--group <name>]... [--rounds N] [--cpu N] [--stop]
       cargo xtask unit-bench compare --reference <file.json> --result <file.json> [--tolerance 0.02]

--stop pauses and restores an acquiring unit.";

pub(super) enum Options<'a> {
    Help,
    Run(Run<'a>),
    Compare(Compare),
}

pub(super) struct Run<'a> {
    pub(super) unit: &'a str,
    pub(super) fixtures: PathBuf,
    pub(super) out: PathBuf,
    pub(super) groups: Vec<&'a str>,
    pub(super) rounds: usize,
    pub(super) cpu: u32,
    pub(super) stop: bool,
}

pub(super) struct Compare {
    pub(super) reference: PathBuf,
    pub(super) result: PathBuf,
    pub(super) tolerance: f64,
}

impl<'a> Options<'a> {
    pub(super) fn parse(args: &[&'a str]) -> Result<Self, String> {
        match args {
            ["--help" | "-h"] | ["run" | "compare", "--help" | "-h"] => Ok(Self::Help),
            ["run", args @ ..] => Run::parse(args).map(Self::Run),
            ["compare", args @ ..] => Compare::parse(args).map(Self::Compare),
            _ => Err("expected run or compare".into()),
        }
    }
}

#[derive(Default)]
struct Flags<'a>(BTreeMap<&'a str, Vec<&'a str>>);

impl<'a> Flags<'a> {
    fn parse(mut args: &[&'a str], allowed: &[&str], switch: Option<&str>) -> Result<Self, String> {
        let mut flags = Self::default();
        while let [flag, rest @ ..] = args {
            if !allowed.contains(flag) {
                return Err("unknown option or unexpected argument".into());
            }
            let values = flags.0.entry(flag).or_default();
            if !values.is_empty() && *flag != "--group" {
                return Err(format!("{flag} was specified more than once"));
            }
            if switch == Some(*flag) {
                values.push("");
                args = rest;
            } else if let [value, remaining @ ..] = rest {
                if value.is_empty() || value.starts_with("--") {
                    return Err(format!("{flag} requires a value"));
                }
                values.push(value);
                args = remaining;
            } else {
                return Err(format!("{flag} requires a value"));
            }
        }
        Ok(flags)
    }

    fn get(&self, flag: &str) -> Option<&'a str> {
        self.0.get(flag)?.first().copied()
    }

    fn required(&self, flag: &str) -> Result<&'a str, String> {
        self.get(flag).ok_or_else(|| format!("{flag} is required"))
    }
}

impl<'a> Run<'a> {
    fn parse(args: &[&'a str]) -> Result<Self, String> {
        let allowed = ["--unit", "--fixtures", "--out", "--group", "--rounds", "--cpu", "--stop"];
        let mut flags = Flags::parse(args, &allowed, Some("--stop"))?;
        let unit = flags.required("--unit")?;
        if unit.starts_with('-') || unit.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err("invalid SSH destination".into());
        }
        let rounds = flags
            .get("--rounds")
            .map(str::parse)
            .unwrap_or(Ok(DEFAULT_ROUNDS))
            .ok()
            .filter(|&n| n > 0)
            .ok_or("--rounds must be a positive integer")?;
        let cpu = flags.get("--cpu").unwrap_or("2").parse().map_err(|_| "--cpu must be a nonnegative integer")?;
        Ok(Self {
            unit,
            fixtures: flags.required("--fixtures")?.into(),
            out: flags.required("--out")?.into(),
            groups: flags.0.remove("--group").unwrap_or_default(),
            rounds,
            cpu,
            stop: flags.get("--stop").is_some(),
        })
    }
}

impl Compare {
    fn parse(args: &[&str]) -> Result<Self, String> {
        let flags = Flags::parse(args, &["--reference", "--result", "--tolerance"], None)?;
        let tolerance = flags
            .get("--tolerance")
            .unwrap_or("0.02")
            .parse::<f64>()
            .ok()
            .filter(|&n| n.is_finite() && n >= 0.0)
            .ok_or("--tolerance must be a finite, nonnegative fraction")?;
        Ok(Self {
            reference: flags.required("--reference")?.into(),
            result: flags.required("--result")?.into(),
            tolerance,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_defaults_and_repeated_groups() {
        let args = ["run", "--unit", "example.invalid", "--fixtures", "fixtures", "--out", "data.json"];
        let Options::Run(run) = Options::parse(&args).unwrap() else { panic!("expected run") };
        assert_eq!((run.rounds, run.cpu, run.stop), (5, 2, false));
        let args = [args.as_slice(), &["--group", "short", "--stop", "--group", "dense"]].concat();
        let Options::Run(run) = Options::parse(&args).unwrap() else { panic!("expected run") };
        assert_eq!(run.groups, ["short", "dense"]);
        assert!(run.stop);
    }

    #[test]
    fn invalid_flags_are_reported_without_echoing_untrusted_arguments() {
        for args in [
            &["run", "--unit", "-unit"][..],
            &["run", "--unit", "example.invalid", "unexpected.example.invalid"],
            &["run", "--stop", "--stop"],
            &["run", "--unit"],
            &["compare", "--tolerance", "NaN"],
        ] {
            let error = Options::parse(args).err().expect("invalid options");
            assert!(!error.contains("example.invalid"), "{error}");
        }
    }
}
