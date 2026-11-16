// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! The kept core offsets (`coretrim::Overlay`) in OOS's own cal directory: whether they apply, loading them, and the
//! message when saving them fails.

use oos_hal::dho::{calfile, coretrim};

/// The core offsets laid over cal_adc: the kept file's with `cal.core_offset = tight`, none with the key off (the file
/// is then not even read, so the key off is a full undo).
pub(super) fn in_use(tight: bool, load: impl FnOnce() -> Option<coretrim::Overlay>) -> coretrim::Overlay {
    if tight {
        load().unwrap_or_default()
    } else {
        coretrim::Overlay::default()
    }
}

/// The kept core offsets from OOS's cal directory; None without a valid file.
pub(super) fn load() -> Option<coretrim::Overlay> {
    let (b, _) = calfile::load_first(coretrim::OVERLAY_FILE, &[calfile::OOS_CAL_DIR])?;
    coretrim::Overlay::from_bytes(calfile::parse(&b).ok()?)
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn save_error(error: &calfile::SaveError) -> String {
    let status = match error {
        calfile::SaveError::Failed(_) => "not saved",
        calfile::SaveError::PublishedDirectorySync(_) => "warning",
    };
    format!("oos-ui hw: core offsets: {}/{} {}: {}", calfile::OOS_CAL_DIR, coretrim::OVERLAY_FILE, status, error)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Break it catches: the kept offsets applied with the key off (no full undo), or read at all then.
    #[test]
    fn overlay_only_with_the_key_on() {
        let mut file = coretrim::Overlay::default();
        file.set(2, &[1; 8]);
        assert_eq!(in_use(false, || panic!("read with the key off")), coretrim::Overlay::default());
        assert_eq!(in_use(true, || Some(file.clone())), file);
        assert_eq!(in_use(true, || None).modes, 0);
    }

    #[test]
    fn overlay_save_error_distinguishes_published_sync_failure() {
        let published = save_error(&calfile::SaveError::PublishedDirectorySync(std::io::Error::other("sync failed")));
        assert!(published.contains("warning: calibration files published; directory fsync failed: sync failed"));
        assert!(!published.contains("not saved"));
        let failed = save_error(&calfile::SaveError::Failed(std::io::Error::other("publish failed")));
        assert!(failed.contains("not saved: publish failed"));
        assert!(!failed.contains("published"));
    }
}
