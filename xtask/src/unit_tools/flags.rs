// SPDX-License-Identifier: GPL-2.0-or-later

use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct Flags<'a>(BTreeMap<&'a str, Vec<&'a str>>);

impl<'a> Flags<'a> {
    pub(crate) fn parse(mut args: &[&'a str], allowed: &[&str], switch: Option<&str>) -> Result<Self, String> {
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

    pub(crate) fn get(&self, flag: &str) -> Option<&'a str> {
        self.0.get(flag)?.first().copied()
    }

    pub(crate) fn groups(&mut self) -> Vec<&'a str> {
        self.0.remove("--group").unwrap_or_default()
    }

    pub(crate) fn required(&self, flag: &str) -> Result<&'a str, String> {
        self.get(flag).ok_or_else(|| format!("{flag} is required"))
    }
}
