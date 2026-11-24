// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Julian Decker

use super::*;

#[derive(Clone, Copy)]
pub(super) struct Selection {
    pub(super) source: WaveId,
    /// 0 BYTE, 1 WORD, 2 ASCII.
    pub(super) format: u8,
    /// 0 NORMal (screen), 1 MAXimum (memory while stopped), 2 RAW (memory).
    pub(super) mode: u8,
    /// 1-based inclusive. A stop of 0 reads through the last point.
    pub(super) start: u64,
    pub(super) stop: u64,
}

#[derive(Default)]
pub(super) struct WaveformContext {
    current: Selection,
    resumed: Option<Selection>,
}

impl Default for Selection {
    fn default() -> Self {
        Self { source: WaveId::Ch(0), format: 0, mode: 0, start: 1, stop: 0 }
    }
}

impl WaveformContext {
    pub(super) fn read(&self) -> Selection {
        self.resumed.unwrap_or(self.current)
    }

    pub(super) fn edit(&mut self, change: impl Fn(&mut Selection)) {
        change(&mut self.current);
        if let Some(selection) = &mut self.resumed {
            change(selection);
        }
    }

    pub(super) fn resume(&mut self, selection: Option<Selection>) {
        self.resumed = selection;
    }
}

pub(super) fn exec(scpi: &mut Scpi, ui: &mut Ui, cmd: &str, nodes: &[&str], query: bool, arg: &str) -> Option<Vec<u8>> {
    let n0 = nodes.first().copied().unwrap_or("");
    if kw(n0, "WAVeform") && nodes.len() == 2 {
        let n1 = nodes[1];
        if kw(n1, "SOURce") {
            return source(scpi, query, arg);
        }
        if kw(n1, "MODE") {
            return mode(scpi, query, arg);
        }
        if kw(n1, "FORMat") {
            return format(scpi, query, arg);
        }
        if kw(n1, "STARt") || kw(n1, "STOP") {
            return range(scpi, ui, query, arg, n1);
        }
        if kw(n1, "POINts") {
            return points(scpi, ui, query, arg);
        }
        if kw(n1, "PREamble") && query {
            return text_reply(&scpi.wave_preamble(ui));
        }
        if kw(n1, "DATA") && query {
            return scpi.wave_data(ui);
        }
    }
    super::undefined(scpi, cmd)
}

fn source(scpi: &mut Scpi, query: bool, arg: &str) -> Option<Vec<u8>> {
    if query {
        return text_reply(&scpi.wav_source_text());
    }
    if let Some(id) = parse_wave(arg) {
        scpi.waveform.edit(|wave| wave.source = id);
    } else if !arg.is_empty() {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
    }
    None
}

fn mode(scpi: &mut Scpi, query: bool, arg: &str) -> Option<Vec<u8>> {
    if query {
        return text_reply(["NORM", "MAX", "RAW"][scpi.waveform.read().mode.min(2) as usize]);
    }
    let a = arg.trim();
    let mode = if kw(a, "RAW") {
        2
    } else if kw(a, "MAXimum") {
        1
    } else if kw(a, "NORMal") {
        0
    } else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return None;
    };
    scpi.waveform.edit(|wave| wave.mode = mode);
    None
}

fn format(scpi: &mut Scpi, query: bool, arg: &str) -> Option<Vec<u8>> {
    if query {
        return text_reply(["BYTE", "WORD", "ASC"][scpi.waveform.read().format as usize]);
    }
    let format = if kw(arg.trim(), "BYTE") {
        Some(0)
    } else if kw(arg.trim(), "WORD") {
        Some(1)
    } else if kw(arg.trim(), "ASCii") {
        Some(2)
    } else {
        None
    };
    if let Some(format) = format {
        scpi.waveform.edit(|wave| wave.format = format);
    } else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
    }
    None
}

fn range(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str, n1: &str) -> Option<Vec<u8>> {
    let stop = kw(n1, "STOP");
    let selection = scpi.waveform.read();
    if query {
        let n = scpi.wave_len(ui);
        let v = if stop {
            if selection.stop == 0 {
                n
            } else {
                selection.stop
            }
        } else {
            selection.start
        };
        return text_reply(&v.to_string());
    }
    let Some(p) = arg.trim().parse::<u64>().ok().filter(|&p| p >= 1) else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return None;
    };
    scpi.waveform.edit(|wave| {
        if stop {
            wave.stop = p;
            wave.start = wave.start.min(p);
        } else {
            wave.start = p;
            if wave.stop != 0 {
                wave.stop = wave.stop.max(p);
            }
        }
    });
    None
}

fn points(scpi: &mut Scpi, ui: &mut Ui, query: bool, arg: &str) -> Option<Vec<u8>> {
    if query {
        return text_reply(&scpi.page_count(ui).to_string());
    }
    // Rigol: the count to read, starting at STARt. stop 0 still means "the rest" until this sets it.
    let Some(n) = arg.trim().parse::<u64>().ok().filter(|&n| n >= 1) else {
        scpi.push_err(format!("-224,\"Illegal parameter value\" {}", arg));
        return None;
    };
    scpi.waveform.edit(|wave| wave.stop = wave.start.saturating_add(n - 1));
    None
}
