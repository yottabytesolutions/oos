// SPDX-License-Identifier: GPL-2.0-or-later

use crate::die;

pub(crate) struct Args {
    pub(crate) rest: Vec<String>,
    flags: Vec<String>,
    vals: Vec<(String, String)>,
}

impl Args {
    pub(crate) fn parse() -> Args {
        let mut a = Args { rest: Vec::new(), flags: Vec::new(), vals: Vec::new() };
        let mut it = std::env::args().skip(1);
        while let Some(s) = it.next() {
            match s.as_str() {
                "--execute" | "--downgrade" | "--force" | "--quick" | "--any-model" | "--reinstall" | "--no-kexec"
                | "--new-dt" | "--any-unit" | "--community" | "--untrusted-ok" => a.flags.push(s),
                _ if s.starts_with("--") => {
                    let v = it.next().unwrap_or_else(|| die(&format!("{} needs a value", s)));
                    a.vals.push((s, v));
                }
                _ => a.rest.push(s),
            }
        }
        a
    }
    pub(crate) fn install_opt<'a>(&self, running_unit: &'a str) -> oos_hal::update::InstallOpt<'a> {
        oos_hal::update::InstallOpt {
            running_unit,
            execute: self.flag("--execute"),
            downgrade: self.flag("--downgrade"),
            force: self.flag("--force"),
            any_unit: self.flag("--any-unit"),
        }
    }
    pub(crate) fn kexec_opt(f: oos_hal::update::kexec::Flags, running_unit: &str) -> oos_hal::update::InstallOpt<'_> {
        oos_hal::update::InstallOpt {
            running_unit,
            execute: true,
            downgrade: f.downgrade,
            force: f.force,
            any_unit: f.any_unit,
        }
    }
    /// Only card-file installs may select a simulated identity; hardware uses the running image.
    pub(crate) fn running_unit(&self) -> String {
        use oos_hal::update::unit;
        if self.val("--card").is_some_and(|c| c != "/dev/mmcblk0") {
            return unit::validate(self.val("--unit").unwrap_or(unit::NONE)).unwrap_or_else(|e| die(&e)).into();
        }
        if self.val("--unit").is_some() {
            die("install --unit is for card-file tests only; use --any-unit for a deliberate profile change");
        }
        unit::running().unwrap_or_else(|e| die(&e))
    }
    pub(crate) fn command(&self) -> &str {
        let cmd = self.rest.first().map(String::as_str).unwrap_or("status");
        for flag in ["--community", "--untrusted-ok"] {
            if self.flag(flag) && cmd != "pack" {
                die(&format!("{} is only valid for pack", flag));
            }
        }
        cmd
    }
    pub(crate) fn flag(&self, f: &str) -> bool {
        self.flags.iter().any(|x| x == f)
    }
    pub(crate) fn val(&self, k: &str) -> Option<&str> {
        self.vals.iter().rev().find(|(x, _)| x == k).map(|(_, v)| v.as_str())
    }
}
