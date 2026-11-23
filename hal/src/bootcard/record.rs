// SPDX-License-Identifier: GPL-2.0-or-later

use super::{CardState, Mode, Reason, Status};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;

pub const STATE_PATH: &str = "/tmp/oos-sd.state";
pub const HISTORY_PATH: &str = "/oos/sd-card.state";
const MAX_BYTES: u64 = 512;

/// Read one start-up record. Missing, oversized or invalid records produce an unknown status.
pub fn read_state(path: &Path) -> Status {
    read_bounded(path, MAX_BYTES).as_deref().and_then(decode).unwrap_or_default()
}

pub(super) fn read_bounded(path: &Path, limit: u64) -> Option<String> {
    let mut text = String::new();
    let n = File::open(path).ok()?.take(limit + 1).read_to_string(&mut text).ok()?;
    (n as u64 <= limit).then_some(text)
}

/// Publish a complete record for the UI, including when detection yields an unknown status.
pub fn write_state(path: &Path, status: Status) -> io::Result<()> {
    atomic_write(path, &encode(status), false)
}

pub(super) fn atomic_write(path: &Path, text: &str, durable: bool) -> io::Result<()> {
    let temporary =
        if durable { path.with_extension("new") } else { path.with_extension(format!("{}.new", std::process::id())) };
    let mut options = File::options();
    options.write(true);
    if durable {
        // The boot check is the single history writer. Reuse the temporary file left by a power cut.
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    #[cfg(unix)]
    if durable {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(&temporary)?;
    let written = (|| {
        file.write_all(text.as_bytes())?;
        if durable {
            file.sync_all()?;
        }
        fs::rename(&temporary, path)
    })();
    if let Err(error) = written {
        if let Err(cleanup) = fs::remove_file(&temporary) {
            if cleanup.kind() != io::ErrorKind::NotFound {
                eprintln!("oos-update: clean up SD card record: {cleanup}");
            }
        }
        return Err(error);
    }
    if durable {
        if let Some(dir) = path.parent() {
            File::open(dir)?.sync_all()?;
        }
    }
    Ok(())
}

fn encode(status: Status) -> String {
    format!(
        "mode={}\nexpected={}\nstate={}\nreason={}\nceiling={}\ndegraded_in_a_row={}\nkexec={}\n",
        status.mode.as_str(),
        status.expected.as_str(),
        status.state.as_str(),
        status.reason.as_str(),
        status.ceiling.as_str(),
        status.degraded_in_a_row,
        u8::from(status.kexec),
    )
}

fn decode(text: &str) -> Option<Status> {
    let mut lines = text.lines();
    let mode = Mode::parse(lines.next()?.strip_prefix("mode=")?)?;
    let expected = Mode::parse(lines.next()?.strip_prefix("expected=")?)?;
    let state = lines.next()?.strip_prefix("state=")?;
    let reason = lines.next()?.strip_prefix("reason=")?;
    let state = CardState::ALL.into_iter().find(|s| s.as_str() == state)?;
    let reason = Reason::ALL.into_iter().find(|r| r.as_str() == reason)?;
    let Some(ceiling) = lines.next() else {
        // Old snapshots are readable, but cannot establish the history needed for a warning.
        let state = if matches!(state, CardState::Degraded | CardState::Worse) { CardState::Unverified } else { state };
        return Some(Status { mode, expected, ceiling: expected, state, reason, degraded_in_a_row: 0, kexec: false });
    };
    let ceiling = Mode::parse(ceiling.strip_prefix("ceiling=")?)?;
    let degraded_in_a_row = lines.next()?.strip_prefix("degraded_in_a_row=")?.parse().ok()?;
    let kexec = match lines.next()?.strip_prefix("kexec=")? {
        "1" => true,
        "0" => false,
        _ => return None,
    };
    Some(Status { mode, expected, ceiling, state, reason, degraded_in_a_row, kexec })
}
