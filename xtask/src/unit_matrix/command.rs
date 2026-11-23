// SPDX-License-Identifier: GPL-2.0-or-later

use crate::unit_tools::{scpi::PORT, Flags};
use std::{net::Ipv6Addr, path::PathBuf, time::Duration};

pub(super) const USAGE: &str = "usage: cargo xtask unit-matrix run --scpi <host[:port]> --out <report.json>
           [--group <id>]... [--warmup <s>] [--window <s>] [--windows <n>] [--pace-ms <ms>] [--bench <file>]
       cargo xtask unit-matrix sustained --scpi <host[:port]> --out <report.json>
           --group <id> --minutes <m> [--pace-ms <ms>] [--bench <file>]
       cargo xtask unit-matrix compare --reference <a.json> --result <b.json> [--floor 0.95]

Run defaults to 5 s warm-up and 5 s windows (three per group, one per drift group).
Sustained uses a 10 s warm-up and one 60 s window per minute.
SCPI pacing defaults to 0 ms. A bench file replaces the matrix's bench_setup settings.";

#[derive(Debug)]
pub(super) enum Options<'a> {
    Help,
    Run(Run<'a>),
    Compare { reference: PathBuf, result: PathBuf, floor: f64 },
}

#[derive(Debug)]
pub(super) struct Run<'a> {
    pub(super) host: &'a str,
    pub(super) port: u16,
    pub(super) out: PathBuf,
    pub(super) groups: Vec<&'a str>,
    pub(super) timing: Timing,
    pub(super) pace: Duration,
    pub(super) bench: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Timing {
    pub(super) warmup: Duration,
    pub(super) window: Duration,
    pub(super) windows: Option<usize>,
    pub(super) sustained: bool,
}

impl<'a> Options<'a> {
    pub(super) fn parse(args: &[&'a str]) -> Result<Self, String> {
        match args {
            ["--help" | "-h"] | ["run" | "sustained" | "compare", "--help" | "-h"] => Ok(Self::Help),
            ["run", args @ ..] => Run::parse(args, false).map(Self::Run),
            ["sustained", args @ ..] => Run::parse(args, true).map(Self::Run),
            ["compare", args @ ..] => {
                let flags = Flags::parse(args, &["--reference", "--result", "--floor"], None)?;
                let floor = flags
                    .get("--floor")
                    .unwrap_or("0.95")
                    .parse::<f64>()
                    .ok()
                    .filter(|&n| n.is_finite() && n >= 0.0)
                    .ok_or("--floor must be a finite, nonnegative ratio")?;
                Ok(Self::Compare {
                    reference: flags.required("--reference")?.into(),
                    result: flags.required("--result")?.into(),
                    floor,
                })
            }
            _ => Err("expected run, sustained, or compare".into()),
        }
    }
}

impl<'a> Run<'a> {
    fn parse(args: &[&'a str], sustained: bool) -> Result<Self, String> {
        let allowed = if sustained {
            &["--scpi", "--out", "--group", "--minutes", "--pace-ms", "--bench"][..]
        } else {
            &["--scpi", "--out", "--group", "--warmup", "--window", "--windows", "--pace-ms", "--bench"][..]
        };
        let mut flags = Flags::parse(args, allowed, None)?;
        let (host, port) = endpoint(flags.required("--scpi")?)?;
        let groups = flags.groups();
        let timing = if sustained {
            if groups.len() != 1 {
                return Err("sustained requires exactly one --group".into());
            }
            Timing {
                warmup: Duration::from_secs(10),
                window: Duration::from_secs(60),
                windows: Some(count(flags.required("--minutes")?, "--minutes")?),
                sustained,
            }
        } else {
            Timing {
                warmup: duration(flags.get("--warmup").unwrap_or("5"), "--warmup", true)?,
                window: duration(flags.get("--window").unwrap_or("5"), "--window", false)?,
                windows: flags.get("--windows").map(|value| count(value, "--windows")).transpose()?,
                sustained,
            }
        };
        let pace_ms: u64 =
            flags.get("--pace-ms").unwrap_or("0").parse().map_err(|_| "--pace-ms must be a nonnegative integer")?;
        let pace = Duration::from_millis(pace_ms);
        Ok(Self {
            host,
            port,
            out: flags.required("--out")?.into(),
            groups,
            timing,
            pace,
            bench: flags.get("--bench").map(Into::into),
        })
    }
}

fn count(value: &str, flag: &str) -> Result<usize, String> {
    value.parse().ok().filter(|&value| value > 0).ok_or_else(|| format!("{flag} must be a positive integer"))
}

fn duration(value: &str, flag: &str, allow_zero: bool) -> Result<Duration, String> {
    value
        .parse::<f64>()
        .ok()
        .and_then(|secs| Duration::try_from_secs_f64(secs).ok())
        .filter(|duration| allow_zero || !duration.is_zero())
        .ok_or_else(|| {
            format!("{flag} must be a representable duration in seconds{}", if allow_zero { "" } else { " above zero" })
        })
}

fn endpoint(value: &str) -> Result<(&str, u16), String> {
    let invalid = || "invalid --scpi endpoint".to_owned();
    let (host, suffix) = if let Some(rest) = value.strip_prefix('[') {
        let (host, suffix) = rest.split_once(']').ok_or_else(invalid)?;
        host.parse::<Ipv6Addr>().map_err(|_| invalid())?;
        (host, suffix)
    } else if value.parse::<Ipv6Addr>().is_ok() {
        (value, "")
    } else {
        let boundary = value.find(':').unwrap_or(value.len());
        let (host, suffix) = value.split_at(boundary);
        if !host.bytes().all(|ch| ch.is_ascii_alphanumeric() || b".-_".contains(&ch)) || host.starts_with('-') {
            return Err(invalid());
        }
        (host, suffix)
    };
    let port = if suffix.is_empty() {
        PORT
    } else {
        suffix.strip_prefix(':').and_then(|port| port.parse().ok()).filter(|&port| port != 0).ok_or_else(invalid)?
    };
    if host.is_empty() {
        return Err(invalid());
    }
    Ok((host, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scpi_endpoints_without_resolving_them() {
        for (text, want) in [
            ("example.invalid", ("example.invalid", 5555)),
            ("example.invalid:1234", ("example.invalid", 1234)),
            ("[2001:db8::1]:5556", ("2001:db8::1", 5556)),
            ("2001:db8::1", ("2001:db8::1", 5555)),
        ] {
            assert_eq!(endpoint(text).unwrap(), want);
        }
        for invalid in ["", ":5555", "example.invalid:0", "[example.invalid]", "[::1]5000", "example.invalid:too"] {
            assert!(endpoint(invalid).is_err());
        }
    }

    #[test]
    fn sustained_requires_one_group_and_sets_one_minute_windows() {
        let args = ["sustained", "--scpi", "example.invalid", "--out", "results.json", "--minutes", "2"];
        assert!(Options::parse(&args).is_err());
        let args = [&args[..], &["--group", "deep"]].concat();
        let Options::Run(run) = Options::parse(&args).unwrap() else { panic!("expected sustained run") };
        assert_eq!(run.timing.window, Duration::from_secs(60));
        assert_eq!(run.timing.windows, Some(2));
        assert!(run.timing.sustained);
        assert_eq!(run.pace, Duration::ZERO);
    }

    #[test]
    fn accepts_explicit_pacing_and_bench_for_both_live_commands() {
        for command in ["run", "sustained"] {
            let mut args = vec![command, "--scpi", "example.invalid", "--out", "results.json", "--group", "deep"];
            if command == "sustained" {
                args.extend(["--minutes", "1"]);
            }
            args.extend(["--bench", "fixture.json", "--pace-ms", "150"]);
            let Options::Run(run) = Options::parse(&args).unwrap() else { panic!("expected live run") };
            assert_eq!(run.bench, Some("fixture.json".into()));
            assert_eq!(run.pace, Duration::from_millis(150));
            *args.last_mut().unwrap() = "-1";
            assert!(Options::parse(&args).is_err());
        }
    }
}
