// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Helpers for the synchronous HTTP adapter.

use std::io;

/// Whether a read error only means "nothing now" (try again in the next pass).
pub fn later(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted)
}
