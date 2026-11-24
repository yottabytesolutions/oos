// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::Opt;
use oos_hal::dho::seq::{self, Playback, Settings};
use oos_hal::dho::{afe, tpu};

struct Parser {
    s: Settings,
    app_mask: u32,
    out: String,
    execute: bool,
    no_read: bool,
    raw_too: bool,
    chan: seq::Chan,
    n: Option<u32>,
    at: Option<i64>,
    all: bool,
    capture: Option<Capture>,
}

struct Capture {
    peak: bool,
    factor: u32,
}

impl Capture {
    fn parse(value: Option<&String>) -> Result<Self, String> {
        let usage = "--capture peak:D|decimate:D";
        let (mode, factor) = value.and_then(|value| value.split_once(':')).ok_or(usage)?;
        let peak = match mode {
            "peak" => true,
            "decimate" => false,
            _ => return Err(usage.into()),
        };
        Ok(Self { peak, factor: factor.parse().map_err(|_| usage)? })
    }
}

pub(super) fn parse(args: &[String]) -> Result<Opt, String> {
    let mut p = Parser {
        s: Settings::default(),
        app_mask: 1,
        out: "/tmp/frame.bin".into(),
        execute: false,
        no_read: false,
        raw_too: false,
        chan: seq::Chan { scale: 1.0, offset: 0.0, bw: afe::bw::OFF, ..Settings::default().ch[0] },
        n: None,
        at: None,
        all: false,
        capture: None,
    };
    let mut i = 0;
    while i < args.len() {
        i += if p.value(&args[i], args.get(i + 1))? {
            2
        } else {
            p.flag(&args[i])?;
            1
        };
    }
    p.finish()
}

fn num(v: Option<&String>) -> Result<f64, String> {
    v.ok_or("missing value")?.parse::<f64>().map_err(|e| e.to_string())
}

fn int(v: Option<&String>) -> Result<u32, String> {
    let t = v.ok_or("missing value")?;
    match t.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(h, 16).map_err(|e| e.to_string()),
        None => t.parse().map_err(|e: std::num::ParseIntError| e.to_string()),
    }
}

impl Parser {
    fn value(&mut self, flag: &str, v: Option<&String>) -> Result<bool, String> {
        match flag {
            "--mask" => self.s.mask = int(v)?,
            "--app-mask" => self.app_mask = int(v)?,
            "--tdiv" => self.s.tdiv = num(v)?,
            "--depth" => self.s.depth = num(v)? as i128,
            "--sample-rate" => {
                self.s.sample_rate =
                    Some(v.ok_or("--sample-rate needs Sa/s")?.parse().map_err(|_| "invalid sample rate")?);
            }
            "--scale" => self.chan.scale = num(v)?,
            "--offset" => self.chan.offset = num(v)?,
            "--level" => self.s.level = num(v)?,
            "--src" => self.s.trig_src = int(v)?,
            "--blocks" => self.s.blocks = int(v)? as u8,
            "--frames" => self.s.frames = int(v)?,
            "--compress" => self.s.compress = int(v)?,
            "--capture" => self.capture = Some(Capture::parse(v)?),
            // hardware averaging, not verified on the unit
            "--average" => self.s.mode = oos_hal::dho::acq::Mode::Average { count: int(v)? },
            "--la-thr" => {
                let t = num(v)?;
                self.s.la = Some(oos_hal::dho::la::Set { thr: [t, t], ..self.s.la.unwrap_or_default() });
            }
            "--n" => self.n = Some(int(v)?),
            "--at" => self.at = Some(num(v)? as i64),
            "--out" => self.out = v.ok_or("missing value")?.clone(),
            "--bw" => {
                self.chan.bw = match v.map(|x| x.as_str()) {
                    Some("OFF") => afe::bw::OFF,
                    Some("S20M") | Some("20M") => afe::bw::S20M,
                    Some("AUTO") | Some("Auto") => afe::bw::AUTO,
                    Some(m) => afe::bw::mhz(m.trim_end_matches('M').parse().map_err(|_| format!("bad --bw {}", m))?),
                    None => return Err("missing value".into()),
                }
            }
            "--slope" => {
                self.s.slope = match v.map(|x| x.as_str()) {
                    Some("rise") => tpu::Slope::Rising,
                    Some("fall") => tpu::Slope::Falling,
                    Some("either") => tpu::Slope::Either,
                    _ => return Err("--slope rise|fall|either".into()),
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn flag(&mut self, a: &str) -> Result<(), String> {
        match a {
            "--normal" => self.s.auto = false,
            "--play4" => self.s.playback = Playback::Raw,
            "--play-last" => self.s.playback = Playback::Last,
            "--keep-config" => self.s.blocks = 0,
            "--header" => self.s.header = true,
            "--no-read" => self.no_read = true,
            "--afe" => self.s.afe = true,
            "--la" => self.s.la = Some(self.s.la.unwrap_or_default()),
            "--la-alone" => {
                self.s.la = Some(oos_hal::dho::la::Set { analog_shown: false, ..self.s.la.unwrap_or_default() })
            }
            "--all" => self.all = true,
            "--execute" => self.execute = true,
            "--raw-too" => self.raw_too = true,
            _ => return Err(format!("unknown option {}", a)),
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Opt, String> {
        self.s.ch = [self.chan; 4];
        let whole_record = self.all || (self.capture.is_some() && self.n.is_none() && self.at.is_none());
        self.s.window = (!whole_record).then(|| (self.n.unwrap_or(4000), self.at.unwrap_or(-2000)));
        if let Some(Capture { peak, factor }) = self.capture {
            if self.s.sample_rate.is_some() {
                return Err("--sample-rate cannot be combined with --capture".into());
            }
            if self.s.reduce_capture(peak, factor, true).is_none() {
                let mode = if peak { "peak" } else { "decimate" };
                return Err(format!("incompatible settings for --capture {mode}:{factor}"));
            }
        }
        if !(1..=15).contains(&self.app_mask) {
            return Err("--app-mask 1..0xf".into());
        }
        if self.raw_too && self.s.compress < 2 {
            return Err("--raw-too needs --compress k > 1".into());
        }
        #[cfg(not(target_os = "linux"))]
        let _ = self.out;
        Ok(Opt {
            s: self.s,
            app_mask: self.app_mask,
            #[cfg(target_os = "linux")]
            out: self.out,
            execute: self.execute,
            no_read: self.no_read,
            #[cfg(target_os = "linux")]
            raw_too: self.raw_too,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options() {
        let o = parse(&["--mask", "3", "--scale", "0.5", "--all"].map(String::from)).unwrap();
        assert_eq!(o.s.mask, 3);
        assert_eq!(o.s.ch[1].scale, 0.5);
        assert!(o.s.window.is_none());
        assert!(parse(&["--bogus".to_string()]).is_err());
    }

    #[test]
    fn capture_flag_selects_the_existing_reduced_mode() {
        for capture in ["peak:2", "decimate:4"] {
            let o = parse(&["--capture", capture, "--frames", "3", "--tdiv", "1e-6"].map(String::from)).unwrap();
            assert!(o.s.window.is_none());
            assert_eq!(o.s.frames, 3);
            let oos_hal::dho::acq::Mode::Reduced(mode) = o.s.mode else {
                panic!("expected reduced capture");
            };
            assert_eq!(mode.to_string(), capture);
        }
    }

    #[test]
    fn sample_rate_flag_keeps_record_depth_independent_of_the_time_base() {
        let o = parse(&["--tdiv", "5e-4", "--depth", "70312", "--sample-rate", "12500000", "--all"].map(String::from))
            .unwrap();
        let p = seq::record_geometry(&o.s).unwrap();
        assert_eq!((p.rate, p.memsel), (12_500_000, 70_312));
    }

    #[test]
    fn capture_flag_rejects_unsupported_combinations() {
        for args in [
            &["--capture"][..],
            &["--capture", "raw:2"],
            &["--capture", "peak:3"],
            &["--capture", "peak:2", "--n", "1000"],
            &["--capture", "peak:2", "--compress", "4"],
            &["--capture", "peak:2", "--average", "4"],
            &["--capture", "peak:2", "--mask", "2", "--src", "2"],
            &["--capture", "peak:8", "--mask", "15"],
            &["--capture", "peak:2", "--sample-rate", "625000000"],
        ] {
            let args: Vec<String> = args.iter().map(|arg| (*arg).into()).collect();
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
}
