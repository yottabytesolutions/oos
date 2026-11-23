// SPDX-License-Identifier: GPL-2.0-or-later

use super::process::{self, Scratch, Stderr};
use std::{
    fs::File,
    process::{Command, Stdio},
};

const OPTIONS: &[&str] = &[
    "-T",
    "-o",
    "BatchMode=yes",
    "-o",
    "StrictHostKeyChecking=yes",
    "-o",
    "UpdateHostKeys=no",
    "-o",
    "ControlPath=none",
    "-o",
    "PermitLocalCommand=no",
    "-o",
    "ClearAllForwardings=yes",
    "-o",
    "ConnectTimeout=10",
    "-o",
    "ServerAliveInterval=5",
    "-o",
    "ServerAliveCountMax=2",
];

pub(super) trait Remote {
    fn run(&mut self, operation: &str, script: &str, seconds: u64, input: Option<File>) -> Result<String, String>;

    fn check_cache(&mut self, operation: &str, script: &str) -> Result<String, String> {
        self.run(operation, script, 15, None)
    }
}

pub(super) struct Ssh<'a> {
    unit: &'a str,
    scratch: &'a Scratch,
}

impl<'a> Ssh<'a> {
    pub(super) fn new(unit: &'a str, scratch: &'a Scratch) -> Self {
        Self { unit, scratch }
    }

    pub(super) fn hostname(&self) -> Result<String, String> {
        let mut command = Command::new("ssh");
        command.args(OPTIONS).args(["-G", "--", self.unit]).stdin(Stdio::null());
        let output = process::output(&mut command, self.scratch, "resolve SSH destination", 15, Stderr::Discard)?;
        output
            .lines()
            .find_map(|line| line.strip_prefix("hostname ").filter(|host| !host.is_empty()).map(str::to_owned))
            .ok_or_else(|| "SSH destination has no hostname".into())
    }

    fn command(&self, script: &str) -> Command {
        let quoted = format!("sh -ec '{}'", script.replace('\'', "'\\''"));
        let mut command = Command::new("ssh");
        command.args(OPTIONS).args(["--", self.unit, &quoted]);
        command
    }
}

impl Remote for Ssh<'_> {
    fn run(&mut self, operation: &str, script: &str, seconds: u64, input: Option<File>) -> Result<String, String> {
        let mut command = self.command(script);
        command.stdin(input.map_or_else(Stdio::null, Stdio::from));
        process::output(&mut command, self.scratch, operation, seconds, Stderr::Discard)
    }

    fn check_cache(&mut self, operation: &str, script: &str) -> Result<String, String> {
        let mut command = self.command(script);
        process::output(command.stdin(Stdio::null()), self.scratch, operation, 15, Stderr::OnFailure)
    }
}
