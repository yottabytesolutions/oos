// SPDX-License-Identifier: GPL-2.0-or-later

use oos_hal::bootcard;
use oos_hal::update::{self, kexec};
use std::path::Path;
use std::{fs, io};

pub(crate) fn run() {
    if let Err(error) = record_once() {
        super::super::die(&format!("card-check: write state: {error}"));
    }
}

pub(crate) fn before_restart() {
    // rcS cannot reach card-check if the automatic restart succeeds.
    if let Err(error) = record_once() {
        super::super::tell(&format!("card-check: write state: {error}"));
    }
}

fn record_once() -> io::Result<()> {
    let state = Path::new(bootcard::STATE_PATH);
    once(state, || record(state))
}

fn once(path: &Path, record: impl FnOnce() -> io::Result<()>) -> io::Result<()> {
    // Claim this boot before appending history. A failed publication must not result in a duplicate start.
    match fs::File::create_new(path) {
        Ok(_) => record(),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

fn record(path: &Path) -> io::Result<()> {
    let mounted = std::fs::read_to_string("/proc/mounts")
        .is_ok_and(|text| text.lines().any(|line| line.split_whitespace().nth(1) == Some("/oos")));
    let history_path = mounted.then(|| Path::new(bootcard::HISTORY_PATH));
    let build = update::running_build();
    let kexec = kexec::flags(&kexec::running_cmdline()).is_some();
    let (status, history) = bootcard::record_start(history_path, build.as_deref(), kexec);
    if let Err(error) = history {
        super::super::tell(&format!("card-check: save SD history: {error}"));
    }
    bootcard::write_state(path, status)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kexec_restart_and_return_to_rcs_record_only_once() {
        let directory = crate::oos_update::test_support::TempDir::new();
        let path = directory.path().join("card.state");
        // A publication failure after updating history still consumes the attempt for this boot.
        assert!(once(&path, || Err(io::Error::other("write failed"))).is_err());
        once(&path, || panic!("the SD start was already recorded")).unwrap();
    }
}
