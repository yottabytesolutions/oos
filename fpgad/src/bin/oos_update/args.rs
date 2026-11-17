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
                | "--new-dt" | "--untrusted-ok" => a.flags.push(s),
                "--any-unit" => die("--any-unit was replaced by --any-model"),
                _ if s.starts_with("--") => {
                    let v = it.next().unwrap_or_else(|| die(&format!("{} needs a value", s)));
                    a.vals.push((s, v));
                }
                _ => a.rest.push(s),
            }
        }
        a
    }
    pub(crate) fn install_opt<'a>(&self, running_model: &'a str) -> oos_hal::update::InstallOpt<'a> {
        oos_hal::update::InstallOpt {
            running_model,
            execute: self.flag("--execute"),
            downgrade: self.flag("--downgrade"),
            force: self.flag("--force"),
            any_model: self.flag("--any-model"),
        }
    }
    pub(crate) fn kexec_opt(f: oos_hal::update::kexec::Flags, running_model: &str) -> oos_hal::update::InstallOpt<'_> {
        oos_hal::update::InstallOpt {
            running_model,
            execute: true,
            downgrade: f.downgrade,
            force: f.force,
            any_model: f.any_model,
        }
    }
    /// Only card-file installs may select a simulated identity; hardware uses hardware detection.
    pub(crate) fn running_model(&self, test_card: bool) -> String {
        use oos_hal::update::models;
        if test_card {
            return models::validate(self.val("--model").unwrap_or(models::NONE)).unwrap_or_else(|e| die(&e)).into();
        }
        if self.val("--model").is_some() {
            die("install --model is for card-file tests only; use --any-model for a deliberate model change");
        }
        models::running().unwrap_or_else(|e| die(&e))
    }
    pub(crate) fn command(&self) -> &str {
        let cmd = self.rest.first().map(String::as_str).unwrap_or("status");
        if self.flag("--untrusted-ok") && cmd != "pack" {
            die("--untrusted-ok is only valid for pack");
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
