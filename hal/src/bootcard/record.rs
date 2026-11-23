// SPDX-License-Identifier: GPL-2.0-or-later

use super::{CardState, Mode, Reason, Status};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

pub const STATE_PATH: &str = "/tmp/oos-sd.state";
const MAX_BYTES: u64 = 512;

/// Read one start-up record. Missing, oversized or invalid records produce an unknown status.
pub fn read_state(path: &Path) -> Status {
    let mut text = String::new();
    File::open(path)
        .and_then(|file| file.take(MAX_BYTES + 1).read_to_string(&mut text))
        .ok()
        .filter(|&n| n as u64 <= MAX_BYTES)
        .and_then(|_| decode(&text))
        .unwrap_or_default()
}

/// Publish a complete record for the UI, including when detection yields an unknown status.
pub fn write_state(path: &Path, status: Status) -> io::Result<()> {
    let temporary = path.with_extension(format!("{}.new", std::process::id()));
    let mut file = File::create_new(&temporary)?;
    let written = file.write_all(encode(status).as_bytes());
    if let Err(error) = written.and_then(|()| fs::rename(&temporary, path)) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    Ok(())
}

fn encode(status: Status) -> String {
    format!(
        "mode={}\nexpected={}\nstate={}\nreason={}\n",
        status.mode.as_str(),
        status.expected.as_str(),
        status.state.as_str(),
        status.reason.as_str(),
    )
}

fn decode(text: &str) -> Option<Status> {
    let mut lines = text.lines();
    let mode = lines.next()?.strip_prefix("mode=")?;
    let expected = lines.next()?.strip_prefix("expected=")?;
    let state = lines.next()?.strip_prefix("state=")?;
    let reason = lines.next()?.strip_prefix("reason=")?;
    Some(Status {
        mode: Mode::ALL.into_iter().find(|m| m.as_str() == mode)?,
        expected: [Mode::Hs, Mode::Sdr50, Mode::Sdr104, Mode::Unknown].into_iter().find(|m| m.as_str() == expected)?,
        state: CardState::ALL.into_iter().find(|s| s.as_str() == state)?,
        reason: Reason::ALL.into_iter().find(|r| r.as_str() == reason)?,
    })
}
