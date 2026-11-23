// SPDX-License-Identifier: GPL-2.0-or-later

use crate::unit_tools::Flags;
use std::path::PathBuf;

pub(super) const DEFAULT_ROUNDS: usize = super::report::MIN_ROUNDS;

pub(super) const USAGE: &str = "\
usage: cargo xtask unit-bench run --unit <ssh destination> --fixtures <dir> --out <file.json>
           [--group <name>]... [--rounds N] [--cpu N] [--average-16] [--stop]
       cargo xtask unit-bench ab --unit <ssh destination> --fixtures <dir>
           --reference-checkout <path> --out <file.json>
           [--group <name>]... [--rounds N] [--cpu N] [--tolerance 0.02]
       cargo xtask unit-bench compare --reference <file.json> --result <file.json> [--tolerance 0.02]

ab interleaves at least five rounds per checkout and pauses and restores acquisition if needed.
For run, --stop opts in to pausing and restoring acquisition. Compare stored runs from one session.";

pub(super) enum Options<'a> {
    Help,
    Run(Run<'a>),
    Ab { run: Run<'a>, reference_checkout: PathBuf, tolerance: f64 },
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
    pub(super) average: bool,
}

pub(super) struct Compare {
    pub(super) reference: PathBuf,
    pub(super) result: PathBuf,
    pub(super) tolerance: f64,
}

impl<'a> Options<'a> {
    pub(super) fn parse(args: &[&'a str]) -> Result<Self, String> {
        match args {
            ["--help" | "-h"] | ["run" | "ab" | "compare", "--help" | "-h"] => Ok(Self::Help),
            [mode @ ("run" | "ab"), args @ ..] => Self::capture(args, *mode == "ab"),
            ["compare", args @ ..] => Compare::parse(args).map(Self::Compare),
            _ => Err("expected run, ab, or compare".into()),
        }
    }

    fn capture(args: &[&'a str], ab: bool) -> Result<Self, String> {
        let mut allowed = vec!["--unit", "--fixtures", "--out", "--group", "--rounds", "--cpu"];
        if ab {
            allowed.extend(["--reference-checkout", "--tolerance"]);
        } else {
            allowed.extend(["--stop", "--average-16"]);
        }
        let average = !ab && args.contains(&"--average-16");
        let filtered = args.iter().copied().filter(|arg| !average || *arg != "--average-16").collect::<Vec<_>>();
        if args.len() - filtered.len() > 1 {
            return Err("--average-16 supplied twice".into());
        }
        let mut flags = Flags::parse(&filtered, &allowed, Some("--stop"))?;
        let mut run = Run::parse(&mut flags)?;
        run.average = average;
        if !ab {
            return Ok(Self::Run(run));
        }
        if run.rounds < super::report::MIN_ROUNDS {
            return Err(format!("ab requires at least {} rounds per checkout", super::report::MIN_ROUNDS));
        }
        run.stop = true;
        Ok(Self::Ab {
            run,
            reference_checkout: flags.required("--reference-checkout")?.into(),
            tolerance: tolerance(&flags)?,
        })
    }
}

impl<'a> Run<'a> {
    fn parse(flags: &mut Flags<'a>) -> Result<Self, String> {
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
            groups: flags.groups(),
            rounds,
            cpu,
            stop: flags.get("--stop").is_some(),
            average: false,
        })
    }
}

impl Compare {
    fn parse(args: &[&str]) -> Result<Self, String> {
        let flags = Flags::parse(args, &["--reference", "--result", "--tolerance"], None)?;
        Ok(Self {
            reference: flags.required("--reference")?.into(),
            result: flags.required("--result")?.into(),
            tolerance: tolerance(&flags)?,
        })
    }
}

fn tolerance(flags: &Flags<'_>) -> Result<f64, String> {
    flags
        .get("--tolerance")
        .unwrap_or("0.02")
        .parse::<f64>()
        .ok()
        .filter(|&n| n.is_finite() && n >= 0.0)
        .ok_or_else(|| "--tolerance must be a finite, nonnegative fraction".into())
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
        let args = ["run", "--unit", "example.invalid", "--fixtures", "fixtures", "--out", "data.json", "--average-16"];
        let Options::Run(run) = Options::parse(&args).unwrap() else { panic!("expected run") };
        assert!(run.average);
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

    #[test]
    fn ab_requires_a_reference_and_at_least_five_rounds() {
        let args = ["ab", "--unit", "example.invalid", "--fixtures", "fixtures", "--out", "data.json"];
        assert_eq!(Options::parse(&args).err().unwrap(), "--reference-checkout is required");
        let args = [args.as_slice(), &["--reference-checkout", "reference"]].concat();
        let Options::Ab { run, reference_checkout, tolerance } = Options::parse(&args).unwrap() else {
            panic!("expected ab")
        };
        assert_eq!((run.rounds, run.cpu, run.stop, tolerance), (5, 2, true, 0.02));
        assert_eq!(reference_checkout, PathBuf::from("reference"));
        for (extra, valid) in [(["--rounds", "4"], false), (["--rounds", "6"], true), (["--tolerance", "NaN"], false)] {
            assert_eq!(Options::parse(&[args.as_slice(), &extra].concat()).is_ok(), valid);
        }
    }
}
