// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::math::{MATH_OPS, WINDOWS};
use oos_dsp::decode::Parity;

struct Key {
    name: &'static str,
    read: fn(&Model, usize) -> Option<String>,
    apply: fn(&mut Model, usize, &str) -> Result<(), String>,
}

macro_rules! read_value {
    (value, $field:expr) => {
        Some($field.to_string())
    };
    (named, $field:expr) => {
        Some(name($field))
    };
    (custom, $read:expr) => {
        $read
    };
}

macro_rules! apply_value {
    (custom, $read:expr, $apply:expr) => {
        $apply
    };
    ($kind:ident, $field:expr, $value:expr) => {
        $field = $value
    };
}

macro_rules! keys {
    ($m:ident, $i:ident, $v:ident; $( $name:literal => $kind:ident($read:expr, $apply:expr); )*) => {
        &[$(Key {
            name: $name,
            read: |$m, $i| { let _ = &$m; read_value!($kind, $read) },
            apply: |$m, $i, $v| { apply_value!($kind, $read, $apply); Ok(()) },
        }),*]
    };
}

mod acquisition;
mod analysers;
mod channel;
mod decoder;
mod display;
mod instruments;
mod math;
mod misc;
mod parse;
#[cfg(test)]
mod tests;
use parse::*;

/// Parses a trigger sweep mode from a setup value.
///
/// # Errors
///
/// Returns an error if the value does not match a setup sweep mode.
pub fn parse_sweep(value: &str) -> Result<TrigMode, String> {
    p_enum(value, &TRIG_MODES)
}

enum Section {
    Plain(&'static [Key]),
    Indexed { prefix: &'static str, count: usize, separator: &'static str, keys: &'static [Key] },
    Maths,
    Decoders,
}

use Section::{Decoders, Indexed, Maths, Plain};

// Sections preserve the file order, including selected math/decoder settings before their other slots.
const SECTIONS: &[Section] = &[
    Indexed { prefix: "deskew_ch", count: NCH, separator: "", keys: channel::DESKEW_CH },
    Indexed { prefix: "deskew_ref", count: NREF, separator: "", keys: channel::DESKEW_REF },
    Indexed { prefix: "ch", count: NCH, separator: ".", keys: channel::CHANNEL },
    Plain(acquisition::ACQUISITION),
    Indexed { prefix: "ch", count: NCH, separator: ".", keys: channel::LABEL },
    Plain(display::CURSOR_KEYS),
    Indexed { prefix: "math_expr", count: crate::math::NMATH, separator: "", keys: math::EXPRESSIONS },
    Plain(math::MATH),
    Maths,
    Plain(decoder::DECODER_SELECTION),
    Plain(decoder::DECODER),
    Plain(decoder::BUS),
    Decoders,
    Plain(display::DISPLAY),
    Indexed { prefix: "view", count: crate::ui::VIEWS_MAX, separator: "", keys: display::VIEWS },
    Plain(analysers::ANALYSERS),
    Plain(instruments::INSTRUMENTS),
    Indexed { prefix: "zone", count: 2, separator: "", keys: misc::ZONES },
    Plain(misc::MISC),
    Plain(misc::LEGACY),
];

fn emit(keys: &[Key], m: &Model, i: usize, prefix: &str, kv: &mut dyn FnMut(&str, String)) {
    for key in keys {
        if let Some(value) = (key.read)(m, i) {
            kv(&format!("{prefix}{}", key.name), value);
        }
    }
}

/// The setup as ordered key/value lines, with the legacy omission and formatting rules.
pub fn setup_text(m: &Model) -> String {
    let mut text = String::from("# OOS setup v1: key = value (oos-ui)\n");
    let mut kv = |key: &str, value: String| {
        text.push_str(key);
        text.push_str(" = ");
        text.push_str(&value);
        text.push('\n');
    };
    for section in SECTIONS {
        match section {
            Plain(keys) => emit(keys, m, 0, "", &mut kv),
            Indexed { prefix, count, separator, keys } => {
                for i in 0..*count {
                    emit(keys, m, i, &format!("{prefix}{}{separator}", i + 1), &mut kv);
                }
            }
            Maths => {
                for i in (0..m.maths.len()).filter(|&i| i != m.math_sel) {
                    emit(math::MATH_CHANNEL, m, i, &format!("m{}.", i + 1), &mut kv);
                }
            }
            Decoders => {
                for i in (0..DECODERS).filter(|&i| i != m.dec_sel && m.decs[i].is_some()) {
                    emit(decoder::DECODER, &m.dec_view(i), 0, &format!("d{}.", i + 1), &mut kv);
                }
            }
        }
    }
    text
}

fn apply(keys: &[Key], m: &mut Model, i: usize, field: &str, value: &str) -> Result<bool, String> {
    if let Some(key) = keys.iter().find(|key| key.name == field) {
        (key.apply)(m, i, value)?;
        return Ok(true);
    }
    Ok(false)
}

fn set_deskew(m: &mut Model, reference: bool, suffix: &str, v: &str) -> Result<(), String> {
    let (count, keys) = if reference { (NREF, channel::DESKEW_REF) } else { (NCH, channel::DESKEW_CH) };
    let i = suffix
        .parse::<usize>()
        .ok()
        .and_then(|i| i.checked_sub(1))
        .filter(|&i| i < count)
        .ok_or("invalid deskew source")?;
    (keys[0].apply)(m, i, v)
}

fn set_slot(m: &mut Model, k: &str, v: &str) -> Result<bool, String> {
    if let Some((n, field)) = k.strip_prefix('d').and_then(|r| r.split_once('.')) {
        if let Ok(n) = n.parse::<usize>() {
            let n = n.checked_sub(1).filter(|&n| n < DECODERS).ok_or("unknown decoder")?;
            let mut t = m.dec_view(n);
            set(&mut t, field, v)?;
            m.dec_store(n, t.dec_cfg());
            return Ok(true);
        }
    }
    if let Some((n, field)) = k.strip_prefix('m').and_then(|r| r.split_once('.')) {
        if let Ok(n) = n.parse::<usize>() {
            let i = n.wrapping_sub(1);
            m.maths.get(i).ok_or("unknown math channel")?;
            if !apply(math::MATH_CHANNEL, m, i, field, v)? {
                return Err("unknown key".into());
            }
            return Ok(true);
        }
    }
    if let Some((c, field)) = k.strip_prefix("ch").and_then(|r| r.split_once('.')) {
        let c = c.parse::<usize>().ok().filter(|c| (1..=NCH).contains(c)).ok_or("unknown channel")? - 1;
        if !apply(channel::CHANNEL, m, c, field, v)? {
            apply(channel::LABEL, m, c, field, v)?;
        }
        return Ok(true);
    }
    Ok(false)
}

pub fn set(m: &mut Model, k: &str, v: &str) -> Result<(), String> {
    if let Some((reference, suffix)) =
        k.strip_prefix("deskew_ch").map(|s| (false, s)).or_else(|| k.strip_prefix("deskew_ref").map(|s| (true, s)))
    {
        return set_deskew(m, reference, suffix, v);
    }
    if set_slot(m, k, v)? {
        return Ok(());
    }
    if let Some(suffix) = k.strip_prefix("math_expr") {
        let n = suffix.parse::<usize>().map_err(|_| "math_expr: no number")?;
        m.math_expr.get(n.wrapping_sub(1)).ok_or("unknown math channel")?;
        return (math::EXPRESSIONS[0].apply)(m, n.wrapping_sub(1), v);
    }
    if let Some(i) =
        k.strip_prefix("view").and_then(|i| i.parse::<usize>().ok()).filter(|i| (1..=crate::ui::VIEWS_MAX).contains(i))
    {
        return (display::VIEWS[0].apply)(m, i - 1, v);
    }
    if k == "zone1" || k == "zone2" {
        return (misc::ZONES[0].apply)(m, usize::from(k == "zone2"), v);
    }
    for section in SECTIONS {
        if let Plain(keys) = section {
            if apply(keys, m, 0, k, v)? {
                break;
            }
        }
    }
    Ok(())
}

fn opt_wave_dash(wave: Option<WaveId>) -> String {
    wave.map(|w| w.token()).unwrap_or_else(|| "-".into())
}

fn opt_wave_off(wave: Option<WaveId>) -> String {
    wave.map_or("off".into(), |w| w.token())
}

fn opt_line_off(line: Option<u8>) -> String {
    line.map_or("off".into(), |c| c.to_string())
}
