// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Mode, Reason};

pub(super) fn parse_ios(text: &str) -> Result<Mode, Reason> {
    let value = text.lines().find_map(|line| line.strip_prefix("timing spec:")).ok_or(Reason::MalformedTiming)?;
    let (number, name) = value.trim().split_once(char::is_whitespace).ok_or(Reason::MalformedTiming)?;
    let name = name.trim().strip_prefix('(').and_then(|name| name.strip_suffix(')'));
    if name.is_none_or(|name| name.trim().is_empty() || name.contains(['(', ')'])) {
        return Err(Reason::MalformedTiming);
    }
    // The kernel prints a name as well. The timing number selects our fixed display label.
    match number.parse::<u32>().map_err(|_| Reason::MalformedTiming)? {
        0 => Ok(Mode::Legacy),
        2 => Ok(Mode::Hs),
        3 => Ok(Mode::Sdr12),
        4 => Ok(Mode::Sdr25),
        5 => Ok(Mode::Sdr50),
        6 => Ok(Mode::Sdr104),
        7 => Ok(Mode::Ddr50),
        _ => Err(Reason::UnknownTiming),
    }
}

/// Use the last card event for this host. A removal or unsupported card supersedes any earlier add.
pub(super) fn from_log(text: &str, host: &str) -> Option<Mode> {
    for line in text.lines().rev() {
        let Some(event) = message(line).strip_prefix(host).and_then(|line| line.strip_prefix(": ")) else {
            continue;
        };
        if let Some((card, _)) = event.strip_prefix("new ").and_then(|line| line.split_once(" card at address ")) {
            return added_mode(card);
        }
        if event.starts_with("card ") && event.ends_with(" removed") {
            return None;
        }
    }
    None
}

fn message(line: &str) -> &str {
    let line = line.trim_start();
    let line = line.strip_prefix('<').and_then(|line| line.split_once('>')).map_or(line, |(_, body)| body);
    line.strip_prefix('[').and_then(|line| line.split_once(']')).map_or(line, |(_, body)| body).trim_start()
}

fn added_mode(card: &str) -> Option<Mode> {
    let (speed, kind) = card.rsplit_once(' ').unwrap_or(("", card));
    if !matches!(kind, "SD" | "SDHC" | "SDXC" | "SD-combo" | "SDHC-combo") {
        return None;
    }
    match speed {
        "" => Some(Mode::Legacy),
        "high speed" => Some(Mode::Hs),
        "ultra high speed SDR12" => Some(Mode::Sdr12),
        "ultra high speed SDR25" => Some(Mode::Sdr25),
        "ultra high speed SDR50" => Some(Mode::Sdr50),
        "ultra high speed SDR104" => Some(Mode::Sdr104),
        "ultra high speed DDR50" => Some(Mode::Ddr50),
        _ => None,
    }
}
