// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::selfcal_saved;

// Upstream 3930159f: a decode call with `acq_lock(&out).cores.then_some(..)` in its arguments kept the guard to
// the end of the statement, so `Shared` stayed locked through a raw decode of up to ~24 MB. The loop is Linux-only and needs the
// FPGA, so this checks its source. Break it catches: a lock taken in a decode's arguments.
#[test]
fn no_decode_holds_the_shared_lock() {
    let src =
        [include_str!("../acq/control.rs"), include_str!("../acq/dispatch.rs"), include_str!("../raw.rs")].concat();
    let call = |l: &&str| !l.trim_start().starts_with("//") && !l.contains("contains(");
    let held =
        src.lines().filter(call).filter(|l| l.contains("decode_with(") && l.contains("lock(")).collect::<Vec<_>>();
    assert!(held.is_empty(), "{held:?}");
}

#[test]
fn audit0073_selfcal_reloads_published_files_and_warns_on_sync_failure() {
    use oos_hal::dho::calfile::SaveError;
    for outcome in [
        Ok(vec!["cal_vertical.hex".into()]),
        Err(SaveError::PublishedDirectorySync(std::io::Error::other("sync failed"))),
        Err(SaveError::Failed(std::io::Error::other("publish failed"))),
    ] {
        let published = !matches!(&outcome, Err(SaveError::Failed(_)));
        let warning = matches!(&outcome, Err(SaveError::PublishedDirectorySync(_)));
        let mut reloads = 0;
        let result = selfcal_saved(outcome, || reloads += 1);
        assert_eq!(reloads, usize::from(published));
        assert_eq!(result.is_ok(), published);
        if warning {
            let message = result.unwrap();
            assert!(message.contains("warning:"));
            assert!(message.contains("files published; directory fsync failed: sync failed"));
        } else if !published {
            assert_eq!(result.unwrap_err(), "publish failed");
        }
    }
}
