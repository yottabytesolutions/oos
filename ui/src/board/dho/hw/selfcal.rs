// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

pub(super) fn selfcal_saved(
    result: Result<Vec<String>, oos_hal::dho::calfile::SaveError>,
    reload: impl FnOnce(),
) -> Result<String, String> {
    use oos_hal::dho::calfile::SaveError;
    let message = match result {
        Ok(files) => format!("saved {}", files.join(", ")),
        Err(e @ SaveError::PublishedDirectorySync(_)) => format!("warning: {}", e),
        Err(e) => return Err(e.to_string()),
    };
    // The published tables take effect at the next set-up even when durability is uncertain.
    reload();
    Ok(message)
}
