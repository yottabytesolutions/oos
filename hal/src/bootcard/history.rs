// SPDX-License-Identifier: GPL-2.0-or-later

use super::{probe, record, CardState, Mode, Status};
use crate::update::hash::{hex, sha256, unhex};
use std::collections::VecDeque;
use std::io;
use std::path::Path;

const VERSION: &str = "oos-sd-card 1";
const HISTORY_LEN: usize = 16;
const MAX_BYTES: u64 = 4096;
type Key = [u8; 16];

#[derive(Debug, Eq, PartialEq)]
struct History {
    key: Key,
    best: Mode,
    counts: Counts,
    recent: VecDeque<Start>,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct Counts {
    starts: u32,
    kexec: u32,
    degraded: u32,
    worse: u32,
    degraded_in_a_row: u32,
}

#[derive(Debug, Eq, PartialEq)]
struct Start {
    build: String,
    kexec: bool,
    mode: Mode,
}

/// Probe once, learn the card's best mode, and save the start when persistent storage is mounted.
/// An IO error describes the history write; the boot status is usable independently of that write.
pub fn record_start(path: Option<&Path>, build: Option<&str>, kexec: bool) -> (Status, io::Result<()>) {
    record_with(path, probe::card_key(), probe::detect(), build, kexec)
}

pub(super) fn card_key(cid: &str) -> Option<Key> {
    let cid: [u8; 16] = unhex(cid.trim())?.try_into().ok()?;
    let digest = sha256(&cid);
    digest[..16].try_into().ok()
}

fn record_with(
    path: Option<&Path>,
    key: Option<Key>,
    mut status: Status,
    build: Option<&str>,
    kexec: bool,
) -> (Status, io::Result<()>) {
    status.kexec = kexec;
    let Some((path, key)) = path.zip(key) else {
        return (classify(status, status.mode), Ok(()));
    };
    let mut history = record::read_bounded(path, MAX_BYTES)
        .as_deref()
        .and_then(History::decode)
        .filter(|history| history.key == key)
        .unwrap_or_else(|| History::new(key));
    let status = history.append(status, build);
    (status, record::atomic_write(path, &history.encode(), true))
}

fn classify(mut status: Status, best: Mode) -> Status {
    status.expected = if best.rate() < status.ceiling.rate() { best } else { status.ceiling };
    status.state = if status.mode == Mode::Unknown || status.ceiling == Mode::Unknown {
        CardState::Unknown
    } else if best.rate() < status.ceiling.rate() {
        CardState::Unverified
    } else {
        CardState::compare(status.mode, status.expected)
    };
    status
}

impl History {
    fn new(key: Key) -> Self {
        Self { key, best: Mode::Unknown, counts: Counts::default(), recent: VecDeque::with_capacity(HISTORY_LEN) }
    }

    fn append(&mut self, status: Status, build: Option<&str>) -> Status {
        if status.mode.rate() > self.best.rate() {
            self.best = status.mode;
        }
        let mut status = classify(status, self.best);
        self.counts.observe(status);
        status.degraded_in_a_row = self.counts.degraded_in_a_row;
        if self.recent.len() == HISTORY_LEN {
            self.recent.pop_front();
        }
        let build = build.filter(|build| valid_build(build)).unwrap_or("?").into();
        self.recent.push_back(Start { build, kexec: status.kexec, mode: status.mode });
        status
    }

    fn encode(&self) -> String {
        let c = &self.counts;
        let mut text = format!(
            "{VERSION}\nkey={}\nbest={}\nstarts={}\nkexec={}\ndegraded={}\nworse={}\ndegraded_in_a_row={}\n",
            hex(&self.key),
            self.best.as_str(),
            c.starts,
            c.kexec,
            c.degraded,
            c.worse,
            c.degraded_in_a_row,
        );
        for start in &self.recent {
            let kind = if start.kexec { "kexec" } else { "ordinary" };
            text.push_str("start=");
            text.push_str(kind);
            text.push('\t');
            text.push_str(start.mode.as_str());
            text.push('\t');
            text.push_str(&start.build);
            text.push('\n');
        }
        text.push_str("end\n");
        text
    }

    fn decode(text: &str) -> Option<Self> {
        let mut lines = text.strip_suffix("end\n")?.lines();
        if lines.next()? != VERSION {
            return None;
        }
        let key = unhex(lines.next()?.strip_prefix("key=")?)?.try_into().ok()?;
        let best = Mode::parse(lines.next()?.strip_prefix("best=")?)?;
        let counts = Counts::decode(&mut lines)?;
        let mut recent = VecDeque::with_capacity(HISTORY_LEN);
        for _ in 0..(counts.starts as usize).min(HISTORY_LEN) {
            let start = Start::decode(lines.next()?)?;
            if start.mode.rate() > best.rate() {
                return None;
            }
            recent.push_back(start);
        }
        lines.next().is_none().then_some(Self { key, best, counts, recent })
    }
}

impl Counts {
    fn observe(&mut self, status: Status) {
        self.starts = self.starts.saturating_add(1);
        self.kexec = self.kexec.saturating_add(u32::from(status.kexec));
        self.degraded = self.degraded.saturating_add(u32::from(status.state == CardState::Degraded));
        self.worse = self.worse.saturating_add(u32::from(status.state == CardState::Worse));
        if status.kexec {
            return;
        }
        if matches!(status.state, CardState::Degraded | CardState::Worse) {
            self.degraded_in_a_row = self.degraded_in_a_row.saturating_add(1);
        } else {
            self.degraded_in_a_row = 0;
        }
    }

    fn decode(lines: &mut std::str::Lines<'_>) -> Option<Self> {
        let mut count = |name| lines.next()?.strip_prefix(name)?.parse::<u32>().ok();
        let starts = count("starts=")?;
        let kexec = count("kexec=")?;
        let degraded = count("degraded=")?;
        let worse = count("worse=")?;
        let degraded_in_a_row = count("degraded_in_a_row=")?;
        let slow = degraded.saturating_add(worse);
        (kexec <= starts && slow <= starts && degraded_in_a_row <= slow).then_some(Self {
            starts,
            kexec,
            degraded,
            worse,
            degraded_in_a_row,
        })
    }
}

impl Start {
    fn decode(line: &str) -> Option<Self> {
        let mut fields = line.strip_prefix("start=")?.splitn(3, '\t');
        let kexec = match fields.next()? {
            "ordinary" => false,
            "kexec" => true,
            _ => return None,
        };
        let mode = Mode::parse(fields.next()?)?;
        let build = fields.next()?.to_owned();
        valid_build(&build).then_some(Self { build, kexec, mode })
    }
}

fn valid_build(build: &str) -> bool {
    !build.is_empty() && build.len() <= 128 && build.bytes().all(|byte| byte.is_ascii_graphic() || byte == b' ')
}

#[cfg(test)]
mod tests;
