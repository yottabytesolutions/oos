// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Storage: setup files (`key = value` text of the Model), waveform CSV, PNG screenshots (own encoder: one fixed-Huffman
//! deflate block with matches against the pixel to the left and above, CRC32, Adler32), file names in the storage directory.

#[cfg(test)]
mod codec_tests;
pub(crate) mod durable;
mod setup;
pub(crate) mod startup;
pub use setup::{set, setup_text};
mod import;
use crate::gfx::Canvas;
use crate::model::*;
use crate::wave::Record;
pub use import::{parse_ag, parse_isf, parse_wfm};
#[cfg(test)]
use oos_dsp::decode::Parity;
use std::path::{Path, PathBuf};

/// Default storage directory on the unit (created when missing).
pub const DEFAULT_DIR: &str = "/tmp/oos";
/// Folder on a USB stick (`/bin/oos-usb` mounts it at /mnt/usb).
pub const USB_DIR: &str = "/mnt/usb/OOS";

/// Where CSV, PNG, mask and fast-frame files go.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StoreDest {
    /// USB stick when mounted, otherwise the storage directory
    Auto,
    /// USB stick (`/mnt/usb/OOS`)
    Stick,
    /// The local storage directory, also when a stick is mounted (on the unit `/tmp/oos`, in RAM)
    Local,
}

impl StoreDest {
    pub const ALL: [StoreDest; 3] = [StoreDest::Auto, StoreDest::Stick, StoreDest::Local];

    pub fn name(self) -> &'static str {
        match self {
            StoreDest::Auto => "Auto",
            StoreDest::Stick => "USB",
            StoreDest::Local => "Local",
        }
    }
}

/// A stick is mounted, by the text of /proc/mounts.
pub fn usb_mounted(mounts: &str) -> bool {
    mounts.lines().any(|l| l.split(' ').nth(1) == Some("/mnt/usb"))
}

/// First free `<prefix>-NNN.<ext>` in `dir` (001 .. 999, then the last one is overwritten).
pub fn next_name(dir: &Path, prefix: &str, ext: &str) -> PathBuf {
    let mut p = dir.join(format!("{}-999.{}", prefix, ext));
    for i in 1..1000 {
        let c = dir.join(format!("{}-{:03}.{}", prefix, i, ext));
        // a link counts as taken, also one that points nowhere
        if c.symlink_metadata().is_err() {
            p = c;
            break;
        }
    }
    p
}

/// `<prefix>-<stamp>.<ext>` (`<prefix>-<stamp>-2.<ext>` and on when taken) with a time stamp, else `next_name`.
pub fn next_name_at(dir: &Path, prefix: &str, ext: &str, stamp: Option<&str>) -> PathBuf {
    let Some(st) = stamp else { return next_name(dir, prefix, ext) };
    for i in 1..100 {
        let n = if i == 1 { format!("{}-{}.{}", prefix, st, ext) } else { format!("{}-{}-{}.{}", prefix, st, i, ext) };
        if dir.join(&n).symlink_metadata().is_err() {
            return dir.join(n);
        }
    }
    next_name(dir, prefix, ext)
}

/// File name of a path for toasts ("shot-001.png").
pub fn file_name(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

/// A stick is foreign data and this program runs as root: a link or a device node on it, under a name a save would use,
/// could lead straight to the card. So only a regular file or a new one, and on the stick no link on the way there.
pub fn check_target(path: &Path) -> std::io::Result<()> {
    let refuse = |what: &str| {
        Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("{}: {}", path.display(), what)))
    };
    if path.starts_with("/mnt/usb") {
        for a in path.ancestors().skip(1).take_while(|a| a.starts_with("/mnt/usb") && *a != Path::new("/mnt/usb")) {
            if a.symlink_metadata().is_ok_and(|m| m.file_type().is_symlink()) {
                return refuse("a link on the way there");
            }
        }
    }
    match path.symlink_metadata() {
        Ok(m) if !m.is_file() => refuse("not a regular file"),
        _ => Ok(()),
    }
}

/// Writes `data` to `path`, creating the directory first.
pub fn write(path: &Path, data: &[u8]) -> std::io::Result<()> {
    check_target(path)?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(path, data)
}

/// Parses a setup file on top of the default set-up (keys missing in the file keep their defaults). Errors name the
/// line and the key; unknown keys are ignored.
#[cfg(test)]
pub fn parse_setup(text: &str) -> Result<Model, String> {
    let (m, bad) = parse_setup_lenient(text);
    match bad.into_iter().next() {
        Some(e) => Err(e),
        None => Ok(m),
    }
}

/// `parse_setup` that loads what it can: a line it cannot take (a key of a newer or another build, a protocol not in
/// this one, a value out of range) is left out and named in the list, the rest of the setup stands. A decoder whose
/// protocol is not in this build is switched off, its other settings kept. For recalling a setup and the restart.
pub fn parse_setup_lenient(text: &str) -> (Model, Vec<String>) {
    let mut bad = Vec::new();
    let m = parse_lines(text, &mut bad);
    (m, bad)
}

fn parse_lines(text: &str, bad: &mut Vec<String>) -> Model {
    let mut m = Model::default();
    let mut proto_missing = false;
    for (ln, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            bad.push(format!("line {}: no '='", ln + 1));
            continue;
        };
        let k = k.trim();
        if let Err(e) = set(&mut m, k, v.trim()) {
            proto_missing |= k == "dec_proto";
            bad.push(format!("line {}: {}: {}", ln + 1, k, e));
        }
    }
    if proto_missing {
        m.dec_on = false;
    }
    // a protocol that became a module since the setup was written: its options from the shared decoder rows
    if let (Proto::Module(i), false) =
        (m.dec_proto, text.lines().any(|l| l.split('=').next().is_some_and(|k| k.trim() == "dec_opt")))
    {
        let old = crate::protocols::Legacy {
            baud: m.dec_baud,
            bits: m.dec_bits,
            parity: m.dec_parity,
            canrate: m.dec_canrate,
            canh: m.dec_canh,
            fdrate: m.dec_fdrate,
            arate: m.dec_arate,
            tick: m.dec_tick,
            nibbles: m.dec_nibbles,
        };
        if let Some(o) = crate::protocols::get(i).legacy(&old) {
            m.dec_opt = o.map(|x| x as u8);
            m.dec_opt = crate::protocols::opt_fit(i, m.dec_opt).map(|x| x as u8);
        }
    }
    // a setup from before the density row: the windows and the pane already say which of the four it was
    if !text.lines().any(|l| l.split('=').next().is_some_and(|k| k.trim() == "density")) {
        m.density = match m.win_layout {
            WinLayout::Quad => Density::Four,
            WinLayout::Cols | WinLayout::Rows => Density::Split,
            WinLayout::One if m.pane != 0 => Density::Split,
            WinLayout::One => Density::Scope,
        };
        // two across is not a density; the split is two stacked traces
        if m.win_layout == WinLayout::Cols {
            m.win_layout = WinLayout::Rows;
        }
        if m.density == Density::Split {
            m.split_pane = m.pane;
        }
    }
    if m.zscale > m.tscale {
        m.zscale = m.tscale;
    }
    m
}

/// Display state of a ref slot, kept in a setup file as `ref<N>.<key>` lines. The samples are not part of a setup:
/// a recall restores how the slot looks, and applies it to the slot when that holds a waveform.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct RefLook {
    pub on: Option<bool>,
    pub vdiv: Option<f32>,
    pub offset: Option<f32>,
    pub color: Option<usize>,
    pub label: Option<String>,
}

/// `ref<N>.*` lines for the loaded slots (`None` = empty slot, nothing is written).
pub fn ref_setup_text(looks: &[Option<RefLook>]) -> String {
    let mut s = String::new();
    for (i, l) in looks.iter().enumerate() {
        let Some(l) = l else { continue };
        let mut kv = |k: &str, v: String| s.push_str(&format!("ref{}.{} = {}\n", i + 1, k, v));
        if let Some(v) = l.on {
            kv("on", v.to_string());
        }
        if let Some(v) = l.vdiv {
            kv("vdiv", v.to_string());
        }
        if let Some(v) = l.offset {
            kv("offset", v.to_string());
        }
        if let Some(v) = l.color {
            kv("color", v.to_string());
        }
        if let Some(v) = &l.label {
            kv("name", v.replace(['\r', '\n'], " "));
        }
    }
    s
}

/// The `ref<N>.*` lines of a setup file; a setup from before they existed gives all `None`.
pub fn parse_ref_looks(text: &str, n: usize) -> Vec<Option<RefLook>> {
    let mut out: Vec<Option<RefLook>> = vec![None; n];
    for line in text.lines() {
        let Some((k, v)) = line.trim().split_once('=') else { continue };
        let Some(rest) = k.trim().strip_prefix("ref") else { continue };
        let Some((idx, key)) = rest.split_once('.') else { continue };
        let Some(i) = idx.parse::<usize>().ok().and_then(|i| i.checked_sub(1)).filter(|&i| i < n) else { continue };
        let v = v.trim();
        let l = out[i].get_or_insert_with(RefLook::default);
        match key {
            "on" => l.on = v.parse().ok(),
            "vdiv" => l.vdiv = v.parse().ok().filter(|x: &f32| x.is_finite() && *x > 0.0),
            "offset" => l.offset = v.parse().ok().filter(|x: &f32| x.is_finite()),
            "color" => l.color = v.parse().ok(),
            "name" => l.label = Some(v.to_string()),
            _ => {}
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------- waveform CSV

/// CSV of the enabled channels of a record: `Time (s),CH1 (V),...`, one row per sample, time relative to the
/// trigger; samples without data (NaN) are empty fields.
pub fn wave_csv(rec: &Record, m: &Model) -> String {
    let chans: Vec<usize> = (0..NCH).filter(|&c| m.ch[c].on && !rec.ch[c].is_empty()).collect();
    let n = chans.iter().map(|&c| rec.ch[c].len()).max().unwrap_or(0);
    let mut s = String::with_capacity(n * (16 + 12 * chans.len()) + 64);
    s.push_str("Time (s)");
    for &c in &chans {
        s.push_str(&format!(",CH{} (V)", c + 1));
    }
    s.push('\n');
    for k in 0..n {
        s.push_str(&format!("{:.9e}", rec.t0 + k as f64 * rec.dt));
        for &c in &chans {
            s.push(',');
            if let Some(v) = rec.ch[c].get(k).filter(|v| v.is_finite()) {
                s.push_str(&v.to_string());
            }
        }
        s.push('\n');
    }
    s
}

/// Logic analyser record as CSV: the lines shown, one row at the record start and one for every change of them, so a
/// record of millions of samples stays small and nothing is lost (the levels hold until the next row). The last row is
/// the end of the record.
pub fn la_csv(la: &crate::ui::la::LaRec, lines: &[usize]) -> String {
    let mask = lines.iter().fold(0u16, |m, &n| m | 1 << n);
    let mut s = String::from("Time (s)");
    for n in lines {
        s.push_str(&format!(",D{}", n));
    }
    s.push('\n');
    let row = |s: &mut String, k: usize, w: u16| {
        s.push_str(&format!("{:.9e}", la.t0 + k as f64 * la.dt));
        for &n in lines {
            s.push_str(if w >> n & 1 == 1 { ",1" } else { ",0" });
        }
        s.push('\n');
    };
    let mut prev = None;
    for (k, &w) in la.words.iter().enumerate() {
        if prev != Some(w & mask) {
            row(&mut s, k, w);
            prev = Some(w & mask);
        }
    }
    if let (Some(&w), true) = (la.words.last(), la.words.len() > 1) {
        row(&mut s, la.words.len() - 1, w);
    }
    s
}

/// The record in a plain binary form of OOS, little endian:
/// "OOSW", u32 version 1, u32 number of channels, u64 samples per channel, f64 dt, f64 t0, then per channel its number
/// (u32, 1..4) and the samples as f32 volts.
pub fn wave_bin(rec: &Record, m: &Model) -> Vec<u8> {
    let chans: Vec<usize> = (0..NCH).filter(|&c| m.ch[c].on && !rec.ch[c].is_empty()).collect();
    let n = chans.iter().map(|&c| rec.ch[c].len()).max().unwrap_or(0);
    let mut b = Vec::with_capacity(36 + chans.len() * (4 + 4 * n));
    b.extend_from_slice(b"OOSW");
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&(chans.len() as u32).to_le_bytes());
    b.extend_from_slice(&(n as u64).to_le_bytes());
    b.extend_from_slice(&rec.dt.to_le_bytes());
    b.extend_from_slice(&rec.t0.to_le_bytes());
    for &c in &chans {
        b.extend_from_slice(&(c as u32 + 1).to_le_bytes());
        for i in 0..n {
            b.extend_from_slice(&rec.ch[c].get(i).copied().unwrap_or(f32::NAN).to_le_bytes());
        }
    }
    b
}

/// One trace taken from a file: volts, the sample step, and the time of the first sample.
#[derive(Clone, Debug)]
pub struct WaveTrace {
    pub t0: f64,
    pub dt: f64,
    pub v: Vec<f32>,
    pub label: String,
}

/// Import bounds shared by native readers and remote uploads. They include the
/// supplied 80 MB / 20-million-point captures without unbounded allocations.
pub const MAX_WAVEFORM_BYTES: usize = 128 << 20;
pub const MAX_WAVEFORM_SAMPLES: usize = 32_000_000;

/// File systems of the kernel: their files can be regular and still endless (`/proc/kmsg`), or a device.
const NOT_STORAGE: [&str; 3] = ["/proc", "/sys", "/dev"];

/// The bytes of a waveform file named by a remote client (SCPI `:REFerence:LOAD`, also over VXI-11 and MCP) or found
/// beside the setups, read on the frame loop. Refused: a file of `NOT_STORAGE` (by the path it resolves to), and one
/// larger than the import limit, from its size before anything is read; the read stops one byte past the limit should
/// the file grow meanwhile. A path that is not there or not a regular file is `missing <name>`.
pub fn read_waveform_file(p: &Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let name = file_name(p);
    let missing = || format!("missing {name}");
    let real = p.canonicalize().map_err(|_| missing())?;
    if NOT_STORAGE.iter().any(|d| real.starts_with(d)) {
        return Err(format!("{name}: files under /proc, /sys and /dev are not read"));
    }
    let too_large = || format!("{name} exceeds the {} MiB import limit", MAX_WAVEFORM_BYTES >> 20);
    // before the open: opening a FIFO would wait for a writer
    let meta = std::fs::metadata(&real).map_err(|_| missing())?;
    if !meta.is_file() {
        return Err(missing());
    }
    if meta.len() > MAX_WAVEFORM_BYTES as u64 {
        return Err(too_large());
    }
    let file = std::fs::File::open(&real).map_err(|_| missing())?;
    let mut bytes = Vec::with_capacity(meta.len() as usize);
    file.take(MAX_WAVEFORM_BYTES as u64 + 1).read_to_end(&mut bytes).map_err(|e| format!("{name}: {e}"))?;
    if bytes.len() > MAX_WAVEFORM_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

/// Extensions shown by native and browser waveform import. The header chooses the parser.
pub const WAVE_EXTS: &[&str] = &["wfm", "bin", "csv", "trc", "isf", "tr0", "wfmx"];

/// A stored waveform, recognized by its header:
/// `OOSW`; `:WFM#001`, `:WFM#002` or `:WFM#003`; `:WFMPRE:`; `WAVEDESC`; cookie `AG`;
/// a `$&%#` block (`9601`, `9007`, or `2001`) or a `Title:` / `Plotname:` transient (`Binary:` or `Values:`);
/// XML `WaveformInternal` or `WaveformPatternInternal`; or a CSV time column.
/// Anything else is refused.
pub fn parse_wave(bytes: &[u8], name: &str) -> Result<Vec<WaveTrace>, String> {
    if bytes.len() > MAX_WAVEFORM_BYTES {
        return Err("waveform exceeds the 128 MiB import limit".into());
    }
    if bytes.starts_with(b"OOSW") {
        return parse_oosw(bytes);
    }
    if looks_wfm(bytes) {
        return parse_wfm(bytes).map(|t| vec![t]);
    }
    if looks_isf(bytes) {
        return parse_isf(bytes);
    }
    if looks_wavedesc(bytes) {
        return parse_wavedesc(bytes);
    }
    if looks_ag(bytes) {
        return parse_ag(bytes);
    }
    if looks_tr0(bytes) {
        return parse_tr0(bytes);
    }
    if looks_wfmx(bytes) {
        return parse_wfmx(bytes);
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return parse_wave_csv(text, name);
    }
    Err("that file is not a waveform".into())
}

fn looks_wfm(bytes: &[u8]) -> bool {
    bytes.len() >= 10
        && (bytes[2..].starts_with(b":WFM#") || bytes[2..].starts_with(b"WFM#") || bytes.starts_with(b":WFM#"))
}

fn parse_oosw(b: &[u8]) -> Result<Vec<WaveTrace>, String> {
    if b.len() < 36 || &b[..4] != b"OOSW" {
        return Err("not an OOS waveform".into());
    }
    let ver = u32::from_le_bytes(b[4..8].try_into().unwrap());
    if ver != 1 {
        return Err(format!("OOS waveform version {ver}"));
    }
    let nch = u32::from_le_bytes(b[8..12].try_into().unwrap()) as usize;
    let n = usize::try_from(u64::from_le_bytes(b[12..20].try_into().unwrap()))
        .map_err(|_| "OOS waveform is too large".to_string())?;
    let dt = f64::from_le_bytes(b[20..28].try_into().unwrap());
    let t0 = f64::from_le_bytes(b[28..36].try_into().unwrap());
    if n > MAX_WAVEFORM_SAMPLES {
        return Err("waveform exceeds the 32-million-sample import limit".into());
    }
    if nch == 0 || nch > 8 || n < 2 || !dt.is_finite() || dt <= 0.0 || !t0.is_finite() {
        return Err("that file holds no waveform".into());
    }
    let need = n
        .checked_mul(4)
        .and_then(|bytes| bytes.checked_add(4))
        .and_then(|bytes| bytes.checked_mul(nch))
        .and_then(|bytes| bytes.checked_add(36))
        .ok_or("OOS waveform is too large")?;
    if b.len() < need {
        return Err("truncated OOS waveform".into());
    }
    let mut out = Vec::with_capacity(nch);
    let mut p = 36;
    for _ in 0..nch {
        let ch = u32::from_le_bytes(b[p..p + 4].try_into().unwrap());
        p += 4;
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            v.push(f32::from_le_bytes(b[p..p + 4].try_into().unwrap()));
            p += 4;
        }
        out.push(WaveTrace { t0, dt, v, label: format!("CH{ch}") });
    }
    Ok(out)
}

/// A time column, then one trace per further column. The columns are kept as the samples they become, and a line wider
/// than the reference slots ends the parse: a single line of many columns no longer makes as many traces.
fn parse_wave_csv(text: &str, name: &str) -> Result<Vec<WaveTrace>, String> {
    let mut columns: Vec<Vec<f32>> = Vec::new();
    let mut times: [f64; 2] = [f64::NAN; 2];
    let mut rows = 0usize;
    for line in text.lines() {
        let mut cols = line.split(',').map(|s| s.trim().parse::<f64>().ok());
        let Some(Some(t)) = cols.next() else { continue };
        let mut rest = cols.map(|x| x.unwrap_or(f64::NAN)).peekable();
        if rest.peek().is_none() {
            continue;
        }
        if rows >= MAX_WAVEFORM_SAMPLES {
            return Err("waveform exceeds the 32-million-sample import limit".into());
        }
        let mut k = 0;
        for x in rest {
            if k >= NREF {
                return Err("The file has more traces than the remaining reference slots".into());
            }
            if k == columns.len() {
                // a column that starts on a later line: the lines before it have no sample there
                columns.push(vec![f32::NAN; rows]);
            }
            columns[k].push(x as f32);
            k += 1;
        }
        for c in &mut columns[k..] {
            c.push(f32::NAN);
        }
        if rows < 2 {
            times[rows] = t;
        }
        rows += 1;
    }
    let dt = match times {
        [a, b] if rows >= 2 && b > a => b - a,
        _ => return Err("that file holds no waveform".into()),
    };
    let ntrace = columns.len();
    Ok(columns
        .into_iter()
        .enumerate()
        .map(|(k, v)| {
            let label = if ntrace == 1 { name.into() } else { format!("CH{}", k + 1) };
            WaveTrace { t0: times[0], dt, v, label }
        })
        .collect())
}

fn looks_isf(b: &[u8]) -> bool {
    let s = if b.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    b.get(s..).is_some_and(|x| x.starts_with(b":WFMPRE:") || x.starts_with(b"WFMPRE:"))
}

fn looks_wavedesc(b: &[u8]) -> bool {
    wavedesc_at(b).is_some_and(|i| b.len() >= i + 188 && matches!((b[i + 34], b[i + 35]), (1, 0) | (0, 0)))
}

fn wavedesc_at(b: &[u8]) -> Option<usize> {
    let n = b.len().min(64);
    b[..n].windows(8).position(|w| w == b"WAVEDESC")
}

fn endian_u16(b: &[u8], o: usize, le: bool) -> u16 {
    let u = u16::from_le_bytes(b[o..o + 2].try_into().unwrap());
    if le {
        u
    } else {
        u.swap_bytes()
    }
}

fn endian_u32(b: &[u8], o: usize, le: bool) -> u32 {
    let u = u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    if le {
        u
    } else {
        u.swap_bytes()
    }
}

fn endian_f32(b: &[u8], o: usize, le: bool) -> f32 {
    let mut raw = [0u8; 4];
    raw.copy_from_slice(&b[o..o + 4]);
    if !le {
        raw.reverse();
    }
    f32::from_le_bytes(raw)
}

fn endian_f64(b: &[u8], o: usize, le: bool) -> f64 {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&b[o..o + 8]);
    if !le {
        raw.reverse();
    }
    f64::from_le_bytes(raw)
}

/// `WAVEDESC` block. `volts = code * gain - offset`, time of point 0 is the double at offset 180.
fn parse_wavedesc(b: &[u8]) -> Result<Vec<WaveTrace>, String> {
    let i = wavedesc_at(b).ok_or("that file is not a waveform")?;
    if b.len() < i + 188 {
        return Err("truncated waveform".into());
    }
    let le = match (b[i + 34], b[i + 35]) {
        (1, 0) => true,
        (0, 0) => false,
        _ => return Err("that file is not a waveform".into()),
    };
    let desc = endian_u32(b, i + 36, le) as usize;
    let user = endian_u32(b, i + 40, le) as usize;
    let trig = endian_u32(b, i + 48, le) as usize;
    let ris = endian_u32(b, i + 52, le) as usize;
    let nbytes = endian_u32(b, i + 60, le) as usize;
    let count = endian_u32(b, i + 116, le) as usize;
    if desc < 188 || desc > 8192 || count < 2 || count > MAX_WAVEFORM_SAMPLES {
        return Err("that file holds no waveform".into());
    }
    let bpp = if count > 0 && nbytes % count == 0 && matches!(nbytes / count, 1 | 2 | 4) {
        nbytes / count
    } else {
        match endian_u16(b, i + 32, le) {
            0 => 1,
            1 => 2,
            _ => return Err("that file holds no waveform".into()),
        }
    };
    let gain = endian_f32(b, i + 156, le) as f64;
    let off = endian_f32(b, i + 160, le) as f64;
    let dt = endian_f32(b, i + 176, le) as f64;
    let t0 = endian_f64(b, i + 180, le);
    if !(dt.is_finite() && dt > 0.0 && gain.is_finite() && off.is_finite() && t0.is_finite()) {
        return Err("that file holds no waveform".into());
    }
    let start = i
        .checked_add(desc)
        .and_then(|x| x.checked_add(user))
        .and_then(|x| x.checked_add(trig))
        .and_then(|x| x.checked_add(ris))
        .ok_or("waveform is too large")?;
    let need = count.checked_mul(bpp).ok_or("waveform is too large")?;
    if start.checked_add(need).is_none_or(|end| end > b.len()) {
        return Err("truncated waveform".into());
    }
    let label = cstr(&b[i + 96..i + 112]);
    let mut v = Vec::with_capacity(count);
    for k in 0..count {
        let o = start + k * bpp;
        let code = match bpp {
            1 => b[o] as i8 as f64,
            2 => endian_u16(b, o, le) as i16 as f64,
            _ => endian_f32(b, o, le) as f64,
        };
        v.push((code * gain - off) as f32);
    }
    Ok(vec![WaveTrace { t0, dt, v, label }])
}

fn looks_ag(b: &[u8]) -> bool {
    if b.len() < 12 + 140 || &b[..2] != b"AG" || !b[2].is_ascii_graphic() || !b[3].is_ascii_graphic() {
        return false;
    }
    let nwave = u32::from_le_bytes(b[8..12].try_into().unwrap());
    let hsize = u32::from_le_bytes(b[12..16].try_into().unwrap());
    (1..=64).contains(&nwave) && (140..8192).contains(&hsize)
}

fn block_le(head: &[u8]) -> Option<bool> {
    if head.len() < 16 {
        return None;
    }
    let mark = |o: usize| match &head[o..o + 4] {
        b"\x04\x00\x00\x00" => Some(true),
        b"\x00\x00\x00\x04" => Some(false),
        _ => None,
    };
    mark(0).or_else(|| mark(8))
}

fn looks_raw(b: &[u8]) -> bool {
    b.starts_with(b"Title:")
        && (b.windows(7).any(|w| w == b"Binary:") || b.windows(7).any(|w| w == b"Values:"))
        && b.windows(9).any(|w| w.eq_ignore_ascii_case(b"Plotname:"))
}

fn looks_tr0(b: &[u8]) -> bool {
    if looks_raw(b) {
        return true;
    }
    let head = &b[..b.len().min(8192)];
    let marked = head.windows(4).any(|w| w == b"$&%#");
    let version = head.windows(4).any(|w| matches!(w, b"9601" | b"2001" | b"9007"));
    if !(marked && version) {
        return false;
    }
    block_le(b).is_some() || b.first().is_some_and(u8::is_ascii_digit)
}

/// `$&%#` post file (`9601` / `9007` float32, `2001` float64) or a `Title:` transient with `Binary:` or `Values:`.
fn parse_tr0(b: &[u8]) -> Result<Vec<WaveTrace>, String> {
    if block_le(b).is_some() {
        return parse_tr0_blocks(b);
    }
    if looks_raw(b) {
        return parse_raw(b);
    }
    parse_tr0_ascii(b)
}

fn post_width(text: &str) -> Result<usize, String> {
    if text.contains("2001") {
        Ok(8)
    } else if text.contains("9601") || text.contains("9007") {
        Ok(4)
    } else {
        Err("that file is not a waveform".into())
    }
}

fn post_vars(text: &str) -> Result<(usize, Vec<String>), String> {
    let end = text.find("$&%#").ok_or("that file is not a waveform")?;
    // Name fields are 16 bytes and may end in spaces, so only a trailing newline is dropped.
    let head = text[..end].trim_end_matches(['\r', '\n', '\0']);
    let digits: String = head.chars().skip_while(|c| c.is_whitespace()).take(4).collect();
    if digits.len() != 4 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err("that file is not a waveform".into());
    }
    let nvars: usize = digits.parse().unwrap_or(0);
    if !(2..=64).contains(&nvars) {
        return Err("that file holds no waveform".into());
    }
    let names = if head.len() >= nvars * 16 {
        head.as_bytes()[head.len() - nvars * 16..]
            .chunks(16)
            .map(|c| String::from_utf8_lossy(c).trim().to_string())
            .collect()
    } else {
        (0..nvars).map(|k| if k == 0 { "time".into() } else { format!("CH{k}") }).collect()
    };
    Ok((nvars, names))
}

fn traces_from_columns(cols: &[f64], nvars: usize, names: &[String], time_f32: bool) -> Result<Vec<WaveTrace>, String> {
    if !(2..=64).contains(&nvars) {
        return Err("invalid waveform column count".into());
    }
    let mut n = cols.len();
    if n > 0 && cols[n - 1].abs() >= 1e29 {
        n -= 1;
    }
    let body = if n >= nvars && n % nvars == 0 {
        &cols[..n]
    } else if n > nvars && (n - 1) % nvars == 0 {
        &cols[1..n]
    } else {
        return Err("that file holds no waveform".into());
    };
    let rows = body.len() / nvars;
    if rows < 2 || rows > MAX_WAVEFORM_SAMPLES {
        return Err(if rows < 2 { "that file holds no waveform" } else { "waveform is too large" }.into());
    }
    let t0 = body[0];
    let dt = if time_f32 { (body[(rows - 1) * nvars] - t0) / (rows - 1) as f64 } else { body[nvars] - t0 };
    if !(t0.is_finite() && dt.is_finite() && dt > 0.0) {
        return Err("that file holds no waveform".into());
    }
    let valid_time = |r: usize| {
        let t = body[r * nvars];
        if time_f32 {
            let expected = t0 + r as f64 * dt;
            let tolerance = (dt.abs() * 1e-5).max(t0.abs().max(expected.abs()) * f32::EPSILON as f64 * 2.0).max(1e-45);
            t.is_finite() && expected.is_finite() && (t - expected).abs() <= tolerance
        } else {
            uniform_time(t, t0, dt, r)
        }
    };
    if !(0..rows).all(valid_time) {
        return Err("nonuniform waveform time axis is not supported".into());
    }
    let mut out = Vec::with_capacity(nvars - 1);
    for k in 1..nvars {
        let v: Vec<f32> = (0..rows).map(|r| body[r * nvars + k] as f32).collect();
        let label = names.get(k).cloned().filter(|s| !s.is_empty()).unwrap_or_else(|| format!("CH{k}"));
        out.push(WaveTrace { t0, dt, v, label });
    }
    Ok(out)
}

fn parse_tr0_blocks(b: &[u8]) -> Result<Vec<WaveTrace>, String> {
    let mut pos = 0usize;
    let mut header = String::new();
    let mut data = Vec::new();
    let mut first = true;
    while pos + 16 <= b.len() {
        let le = block_le(&b[pos..pos + 16]).ok_or("that file is not a waveform")?;
        let n = endian_u32(b, pos + 12, le) as usize;
        if n > MAX_WAVEFORM_BYTES || pos + 16 + n > b.len() {
            return Err(if n > MAX_WAVEFORM_BYTES { "waveform is too large" } else { "truncated waveform" }.into());
        }
        let body = &b[pos + 16..pos + 16 + n];
        pos += 16 + n;
        if pos + 4 <= b.len() {
            pos += 4;
        }
        if first {
            header = String::from_utf8_lossy(body).into_owned();
            first = false;
        } else {
            data.extend_from_slice(body);
        }
    }
    if header.find("$&%#").is_none() {
        return Err("that file is not a waveform".into());
    }
    let width = post_width(&header)?;
    let (nvars, names) = post_vars(&header)?;
    if data.len() % width != 0 {
        return Err("truncated waveform".into());
    }
    let mut cols = Vec::with_capacity(data.len() / width);
    for chunk in data.chunks(width) {
        let x = if width == 8 {
            f64::from_le_bytes(chunk.try_into().unwrap())
        } else {
            f32::from_le_bytes(chunk.try_into().unwrap()) as f64
        };
        if x.abs() >= 1e29 {
            break;
        }
        cols.push(x);
    }
    // The block endian applies to the floats as well. Re-read when the header was big endian.
    if block_le(b) == Some(false) {
        cols.clear();
        for chunk in data.chunks(width) {
            let x = if width == 8 { endian_f64(chunk, 0, false) } else { endian_f32(chunk, 0, false) as f64 };
            if x.abs() >= 1e29 {
                break;
            }
            cols.push(x);
        }
    }
    traces_from_columns(&cols, nvars, &names, width == 4)
}

fn parse_tr0_ascii(b: &[u8]) -> Result<Vec<WaveTrace>, String> {
    let text = std::str::from_utf8(b).map_err(|_| "that file is not a waveform".to_string())?;
    let end = text.find("$&%#").ok_or("that file is not a waveform")?;
    let width_bytes = post_width(text)?;
    let (nvars, names) = post_vars(text)?;
    let rest = text[end + 4..].trim_start();
    let field = if width_bytes == 8 { 13 } else { 11 };
    let packed = rest
        .as_bytes()
        .get(..field)
        .and_then(|b| std::str::from_utf8(b).ok())
        .is_some_and(|s| !s.bytes().any(|b| b.is_ascii_whitespace()) && s.trim().parse::<f64>().is_ok());
    let mut cols = Vec::new();
    if packed {
        let bytes = rest.as_bytes();
        let mut i = 0;
        while i + field <= bytes.len() {
            let s = std::str::from_utf8(&bytes[i..i + field]).unwrap_or("").trim();
            if s.is_empty() {
                break;
            }
            let Ok(x) = s.parse::<f64>() else { break };
            if x.abs() >= 1e29 {
                break;
            }
            cols.push(x);
            i += field;
        }
    }
    if cols.len() < nvars * 2 {
        cols.clear();
        for tok in rest.split_whitespace() {
            let Ok(x) = tok.parse::<f64>() else { break };
            if x.abs() >= 1e29 {
                break;
            }
            cols.push(x);
        }
    }
    traces_from_columns(&cols, nvars, &names, false)
}

fn parse_raw(b: &[u8]) -> Result<Vec<WaveTrace>, String> {
    let mark = b.windows(7).position(|w| w == b"Binary:" || w == b"Values:").ok_or("that file is not a waveform")?;
    let header = std::str::from_utf8(&b[..mark]).map_err(|_| "that file is not a waveform".to_string())?;
    let nvars = raw_field(header, "No. Variables:").ok_or("that file holds no waveform")?;
    let npts = raw_field(header, "No. Points:").unwrap_or(0);
    if !(2..=64).contains(&nvars) {
        return Err("that file holds no waveform".into());
    }
    let complex =
        header.to_ascii_lowercase().contains("flags: complex") || header.to_ascii_lowercase().contains("flags:complex");
    let names = raw_names(header, nvars);
    let binary = b[mark..].starts_with(b"Binary:");
    let mut cols = Vec::new();
    if binary {
        let mut i = mark + 7;
        if b.get(i) == Some(&b'\r') {
            i += 1;
        }
        if b.get(i) == Some(&b'\n') {
            i += 1;
        }
        let points = if npts >= 2 { npts } else { (b.len() - i) / (nvars * 8) };
        if points < 2 || points > MAX_WAVEFORM_SAMPLES {
            return Err(if points < 2 { "that file holds no waveform" } else { "waveform is too large" }.into());
        }
        if i + points * nvars * 8 > b.len() {
            return Err("truncated waveform".into());
        }
        for p in 0..points {
            for k in 0..nvars {
                let o = i + (p * nvars + k) * 8;
                let x = if complex { endian_f32(b, o, true) as f64 } else { endian_f64(b, o, true) };
                cols.push(x);
            }
        }
    } else {
        let text = std::str::from_utf8(&b[mark + 7..]).map_err(|_| "that file is not a waveform".to_string())?;
        let nums: Vec<f64> = text
            .split(|c: char| !(c.is_ascii_digit() || c == '-' || c == '+' || c == '.' || c == 'e' || c == 'E'))
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        let stride = nvars + 1;
        let indexed = nums.len() >= stride * 2 && nums[0].abs() < 1e-9 && (nums[stride] - 1.0).abs() < 1e-6;
        if indexed {
            let rows = nums.len() / stride;
            for r in 0..rows {
                cols.extend_from_slice(&nums[r * stride + 1..r * stride + 1 + nvars]);
            }
        } else if nums.len() >= nvars * 2 && nums.len() % nvars == 0 {
            cols = nums;
        } else {
            return Err("that file holds no waveform".into());
        }
    }
    traces_from_columns(&cols, nvars, &names, false)
}

fn raw_field(header: &str, key: &str) -> Option<usize> {
    let lower = header.to_ascii_lowercase();
    let i = lower.find(&key.to_ascii_lowercase())? + key.len();
    let rest = header.get(i..)?.trim_start();
    let n: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    n.parse().ok()
}

fn raw_names(header: &str, nvars: usize) -> Vec<String> {
    let mut names = vec![String::new(); nvars];
    let Some(at) = header.to_ascii_lowercase().find("variables:") else { return names };
    for line in header[at + 9..].lines() {
        let mut it = line.split_whitespace();
        let Some(idx) = it.next().and_then(|s| s.parse::<usize>().ok()) else { continue };
        if let Some(name) = it.next() {
            if idx < nvars {
                names[idx] = name.to_string();
            }
        }
    }
    names
}

fn looks_wfmx(b: &[u8]) -> bool {
    let s = if b.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    let rest = b.get(s..).unwrap_or(b);
    if !rest.starts_with(b"<?xml") {
        return false;
    }
    let n = rest.len().min(8192);
    let t = &rest[..n];
    let has = |needle: &[u8]| t.windows(needle.len()).any(|w| w == needle);
    has(b"WaveformPatternInternal") || has(b"WaveformInternal")
}

fn xml_text<'a>(hay: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let i = hay.find(&open)? + open.len();
    let j = hay[i..].find(&close)? + i;
    Some(hay[i..j].trim())
}

/// XML header `WaveformInternal` or `WaveformPatternInternal`, then float samples. `XOrg` is the first time, `XInc` the step.
fn parse_wfmx(b: &[u8]) -> Result<Vec<WaveTrace>, String> {
    let s = if b.starts_with(&[0xEF, 0xBB, 0xBF]) { 3 } else { 0 };
    let head_end = b[s..].windows(7).position(|w| w == b"</Data>").map(|p| s + p + 7).unwrap_or(b.len().min(s + 8192));
    let head = String::from_utf8_lossy(&b[s..head_end.min(b.len())]);
    if !(head.contains("WaveformInternal") || head.contains("WaveformPatternInternal")) {
        return Err("that file is not a waveform".into());
    }
    let points: usize = xml_text(&head, "Points").and_then(|s| s.parse().ok()).ok_or("that file holds no waveform")?;
    let t0: f64 = xml_text(&head, "XOrg").and_then(|s| s.parse().ok()).ok_or("that file holds no waveform")?;
    let dt: f64 = xml_text(&head, "XInc").and_then(|s| s.parse().ok()).ok_or("that file holds no waveform")?;
    if points < 2 || points > MAX_WAVEFORM_SAMPLES || !(dt.is_finite() && dt > 0.0 && t0.is_finite()) {
        return Err("that file holds no waveform".into());
    }
    let payload = wfmx_payload(b, points)?;
    let v = wfmx_volts(payload, points, t0, dt)?;
    let label = xml_text(&head, "SourceName").unwrap_or("").to_string();
    Ok(vec![WaveTrace { t0, dt, v, label }])
}

fn wfmx_payload(b: &[u8], points: usize) -> Result<&[u8], String> {
    let lengths = [4usize, 8, 16].map(|width| points.checked_mul(width).ok_or("waveform is too large"));
    let [four, eight, sixteen] = lengths;
    let lengths = [four?, eight?, sixteen?];
    let fits = |p: &[u8]| lengths.contains(&p.len());
    if let Some(start) = b.windows(5).enumerate().find_map(|(i, w)| {
        (w == b"<Data" && b.get(i + 5).is_some_and(|c| *c == b'>' || c.is_ascii_whitespace())).then_some(i)
    }) {
        if let Some(gt) = b[start..].iter().position(|&c| c == b'>') {
            let from = start + gt + 1;
            for length in lengths {
                if let Some(end) = from.checked_add(length) {
                    if b.get(end..).is_some_and(|tail| tail.starts_with(b"</Data>")) {
                        return Ok(&b[from..end]);
                    }
                }
            }
        }
    }
    // Samples follow the header. An empty `<Data>` element means they start after
    // the closing root tag. Consume only the XML separator required by the exact
    // payload size; leading/trailing float bytes can themselves be whitespace.
    if let Some(i) = b.windows(7).position(|w| w == b"</Data>") {
        let mut p = i + 7;
        let mut root = p;
        while b.get(root).is_some_and(u8::is_ascii_whitespace) {
            root += 1;
        }
        if b.get(root) == Some(&b'<') && b.get(root + 1) == Some(&b'/') {
            if let Some(gt) = b[root..].iter().position(|&c| c == b'>') {
                p = root + gt + 1;
            }
        }
        let tail = &b[p..];
        // A single XML line ending is supported after the root. Never trim the
        // binary tail or skip an unbounded run of its possible first bytes. A
        // present separator is structural, never a replacement for missing data.
        for separator in [b"\r\n".as_slice(), b"\n".as_slice()] {
            if let Some(payload) = tail.strip_prefix(separator) {
                return if fits(payload) {
                    Ok(payload)
                } else {
                    Err("truncated or unsupported waveform payload".into())
                };
            }
        }
        if fits(tail) {
            return Ok(tail);
        }
    }
    Err("that file holds no waveform".into())
}

fn wfmx_volts(payload: &[u8], points: usize, t0: f64, dt: f64) -> Result<Vec<f32>, String> {
    let le_f32 = |p: &[u8], be: bool| -> Vec<f32> { (0..p.len() / 4).map(|k| endian_f32(p, k * 4, !be)).collect() };
    let as_y = |ys: &[f32]| ys.iter().take(points).copied().collect::<Vec<_>>();
    if payload.len() == points * 4 {
        let v = le_f32(payload, false);
        if v.iter().filter(|x| x.is_finite()).count() * 2 >= v.len() {
            return Ok(v);
        }
        return Ok(le_f32(payload, true));
    }
    if payload.len() == points * 8 {
        let f32s = le_f32(payload, false);
        let xs: Vec<f64> = f32s[..points].iter().map(|x| *x as f64).collect();
        if x_matches_f32(&xs, t0, dt) {
            return Ok(as_y(&f32s[points..]));
        }
        let inter: Vec<f64> = f32s.iter().step_by(2).copied().map(|x| x as f64).collect();
        if x_matches_f32(&inter, t0, dt) {
            return Ok(f32s.iter().skip(1).step_by(2).copied().collect());
        }
        // Eight bytes per point could also be an implicit f64 Y array. Without
        // a declared layout it cannot be distinguished safely from malformed
        // f32 X/Y data. Never reinterpret a failed explicit time axis as volts.
        return Err("unsupported or ambiguous WFMX sample layout".into());
    }
    if payload.len() == points * 16 {
        let xs: Vec<f64> = (0..points).map(|k| endian_f64(payload, k * 8, true)).collect();
        if x_matches(&xs, t0, dt) {
            return Ok((0..points).map(|k| endian_f64(payload, (points + k) * 8, true) as f32).collect());
        }
        let inter: Vec<f64> = (0..points).map(|k| endian_f64(payload, k * 16, true)).collect();
        if x_matches(&inter, t0, dt) {
            return Ok((0..points).map(|k| endian_f64(payload, k * 16 + 8, true) as f32).collect());
        }
    }
    Err("that file holds no waveform".into())
}

fn x_matches_f32(xs: &[f64], t0: f64, dt: f64) -> bool {
    xs.len() >= 2
        && xs.iter().enumerate().all(|(i, &x)| {
            let expected = t0 + i as f64 * dt;
            let tolerance = (dt.abs() * 1e-5).max(expected.abs() * f32::EPSILON as f64).max(1e-45);
            x.is_finite() && expected.is_finite() && (x - expected).abs() <= tolerance
        })
}

fn x_matches(xs: &[f64], t0: f64, dt: f64) -> bool {
    xs.len() >= 2 && xs.iter().enumerate().all(|(i, &x)| uniform_time(x, t0, dt, i))
}

/// A WaveTrace has one interval; adaptive/invalid time axes cannot be relabeled
/// uniform without moving their samples. Tolerance covers source float rounding.
fn uniform_time(time: f64, t0: f64, dt: f64, index: usize) -> bool {
    let expected = t0 + index as f64 * dt;
    let tolerance = (dt.abs() * 1e-5).max(expected.abs() * f64::EPSILON * 8.0).max(1e-18);
    time.is_finite() && expected.is_finite() && (time - expected).abs() <= tolerance
}

fn cstr(b: &[u8]) -> String {
    b.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect::<String>().trim().to_string()
}

fn u32_le(b: &[u8], at: usize) -> Option<u32> {
    b.get(at..at + 4)?.try_into().ok().map(u32::from_le_bytes)
}

fn u64_le(b: &[u8], at: usize) -> Option<u64> {
    b.get(at..at + 8)?.try_into().ok().map(u64::from_le_bytes)
}

fn f64_le(b: &[u8], at: usize) -> Option<f64> {
    b.get(at..at + 8)?.try_into().ok().map(f64::from_le_bytes)
}

fn f32_le(b: &[u8], at: usize) -> Option<f32> {
    b.get(at..at + 4)?.try_into().ok().map(f32::from_le_bytes)
}

/// One OOSW blob: the record and the number of bytes consumed.
pub fn parse_wave_bin(b: &[u8]) -> Result<(Record, usize), String> {
    if b.len() < 36 || &b[..4] != b"OOSW" {
        return Err("not an OOS waveform file".into());
    }
    let ver = u32_le(b, 4).unwrap_or(0);
    if ver != 1 {
        return Err(format!("waveform version {}", ver));
    }
    let nch = u32_le(b, 8).unwrap_or(0) as usize;
    // the sample count comes from the file: bounded before it sizes anything, so a damaged or hostile count is an
    // error and not an overflow or an allocation the size of the count
    let n = usize::try_from(u64_le(b, 12).unwrap_or(0)).unwrap_or(usize::MAX);
    let dt = f64_le(b, 20).unwrap_or(0.0);
    let t0 = f64_le(b, 28).unwrap_or(0.0);
    if nch == 0 || nch > NCH || dt <= 0.0 {
        return Err("waveform header".into());
    }
    if n > MAX_WAVEFORM_SAMPLES {
        return Err("waveform exceeds the 32-million-sample import limit".into());
    }
    let mut off = 36;
    let mut rec =
        Record { t0, dt, ch: Default::default(), triggered: true, partial: false, raw: None, la: None, pairs: false };
    for _ in 0..nch {
        let c = u32_le(b, off).ok_or("truncated waveform")? as usize;
        off += 4;
        if c < 1 || c > NCH {
            return Err(format!("channel {}", c));
        }
        if b.len().saturating_sub(off) < 4 * n {
            return Err("truncated waveform".into());
        }
        let mut v = vec![0f32; n];
        for i in 0..n {
            v[i] = f32_le(b, off + 4 * i).unwrap_or(f32::NAN);
        }
        rec.ch[c - 1] = v;
        off += 4 * n;
    }
    Ok((rec, off))
}

/// Fast-frame file: "OOSF", u32 version 1, u32 frame count, then that many OOSW blobs.
pub fn ff_bin(recs: &[Record], m: &Model) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(b"OOSF");
    b.extend_from_slice(&1u32.to_le_bytes());
    b.extend_from_slice(&(recs.len() as u32).to_le_bytes());
    for rec in recs {
        b.extend_from_slice(&wave_bin(rec, m));
    }
    b
}

pub fn parse_ff(b: &[u8]) -> Result<Vec<Record>, String> {
    if b.len() >= 4 && &b[..4] == b"OOSW" {
        return parse_wave_bin(b).map(|(r, _)| vec![r]);
    }
    if b.len() < 12 || &b[..4] != b"OOSF" {
        return Err("not a fast-frame file".into());
    }
    let ver = u32_le(b, 4).unwrap_or(0);
    if ver != 1 {
        return Err(format!("fast-frame version {}", ver));
    }
    let n = u32_le(b, 8).unwrap_or(0) as usize;
    // every frame is at least a header and one channel number: a count the bytes cannot hold is refused before it
    // sizes the list
    if n > (b.len() - 12) / 40 {
        return Err("truncated fast-frame file".into());
    }
    let mut off = 12;
    let mut recs = Vec::with_capacity(n);
    for _ in 0..n {
        let (r, used) = parse_wave_bin(b.get(off..).unwrap_or(&[]))?;
        recs.push(r);
        off += used;
    }
    Ok(recs)
}

/// Highest numbered `<prefix>-NNN.<ext>` in `dir`, if any.
pub fn latest_name(dir: &Path, prefix: &str, ext: &str) -> Option<PathBuf> {
    let mut best: Option<(u32, PathBuf)> = None;
    let rd = std::fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        let s = e.file_name().to_string_lossy().into_owned();
        let want = format!("{}-", prefix);
        if let Some(rest) = s.strip_prefix(&want).and_then(|r| r.strip_suffix(&format!(".{}", ext))) {
            if let Ok(n) = rest.parse::<u32>() {
                if best.as_ref().map_or(true, |(b, _)| n >= *b) {
                    best = Some((n, e.path()));
                }
            }
        }
    }
    best.map(|x| x.1)
}

/// Mask file: optional header (`# OOS mask` then `key=value` lines and `---`) then one volt per line.
pub struct MaskFile {
    pub src: Option<usize>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub inv: Option<bool>,
    pub v: Vec<f32>,
}

pub fn mask_text(src: usize, x: f32, y: f32, inv: bool, v: &[f32]) -> String {
    let mut s = String::from("# OOS mask\n");
    s.push_str(&format!("src={}\n", src + 1));
    s.push_str(&format!("x={}\n", x));
    s.push_str(&format!("y={}\n", y));
    s.push_str(&format!("inv={}\n", if inv { 1 } else { 0 }));
    s.push_str("---\n");
    for x in v {
        s.push_str(&format!("{:.9e}\n", x));
    }
    s
}

pub fn parse_mask(text: &str) -> Result<MaskFile, String> {
    let mut lines = text.lines().peekable();
    let mut src = None;
    let mut x = None;
    let mut y = None;
    let mut inv = None;
    if lines.peek().map_or(false, |l| l.starts_with("# OOS mask")) {
        lines.next();
        for line in lines.by_ref() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == "---" {
                break;
            }
            let Some((k, v)) = line.split_once('=') else {
                return Err(format!("mask header: {}", line));
            };
            match k.trim() {
                "src" => {
                    let n: usize = v.trim().parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
                    if n < 1 || n > NCH {
                        return Err(format!("mask src {}", n));
                    }
                    src = Some(n - 1);
                }
                "x" => x = Some(v.trim().parse().map_err(|e: std::num::ParseFloatError| e.to_string())?),
                "y" => y = Some(v.trim().parse().map_err(|e: std::num::ParseFloatError| e.to_string())?),
                "inv" => inv = Some(matches!(v.trim(), "1" | "true" | "on")),
                _ => {}
            }
        }
    }
    let mut v = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        v.push(line.parse::<f32>().map_err(|e| e.to_string())?);
    }
    if v.is_empty() {
        return Err("empty mask".into());
    }
    Ok(MaskFile { src, x, y, inv, v })
}

// ---------------------------------------------------------------------------------------------- PNG

/// CRC-32 tables for eight bytes per step ("slicing by 8"): `T[0]` is the usual byte table, `T[k][n]` the CRC of byte
/// `n` followed by `k` zero bytes.
const CRC_T: [[u32; 256]; 8] = {
    let mut t = [[0u32; 256]; 8];
    let mut n = 0;
    while n < 256 {
        let mut c = n as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[0][n] = c;
        n += 1;
    }
    let mut k = 1;
    while k < 8 {
        let mut n = 0;
        while n < 256 {
            let c = t[k - 1][n];
            t[k][n] = t[0][(c & 0xff) as usize] ^ (c >> 8);
            n += 1;
        }
        k += 1;
    }
    t
};

/// CRC-32 (ISO 3309 / PNG / zip).
pub fn crc32(data: &[u8]) -> u32 {
    let t = &CRC_T;
    let mut c = !0u32;
    let mut it = data.chunks_exact(8);
    for b in &mut it {
        let lo = c ^ u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        c = t[7][(lo & 0xff) as usize]
            ^ t[6][(lo >> 8 & 0xff) as usize]
            ^ t[5][(lo >> 16 & 0xff) as usize]
            ^ t[4][(lo >> 24) as usize]
            ^ t[3][b[4] as usize]
            ^ t[2][b[5] as usize]
            ^ t[1][b[6] as usize]
            ^ t[0][b[7] as usize];
    }
    !it.remainder().iter().fold(c, |c, &b| t[0][((c ^ b as u32) & 0xff) as usize] ^ (c >> 8))
}

/// Adler-32 (zlib). Sixteen lanes side by side (the compiler makes vector code of the lane loop): lane `k` sums the
/// bytes at `16 j + k` (`sa`) and those sums again (`sb`), and per chunk of 5552 bytes `b` grows by `len * a`,
/// `16 * sum(sb)` less `sum(k * sa[k])` (each byte counted `len - position` times), the same as byte by byte.
pub fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u64, 0u64);
    for chunk in data.chunks(5552) {
        let (mut sa, mut sb) = ([0u32; 16], [0u32; 16]);
        let mut blocks = chunk.chunks_exact(16);
        for blk in &mut blocks {
            for k in 0..16 {
                sa[k] += blk[k] as u32;
                sb[k] += sa[k];
            }
        }
        let n = (chunk.len() - blocks.remainder().len()) as u64;
        let sum_a: u64 = sa.iter().map(|&x| x as u64).sum();
        let sum_b: u64 = sb.iter().map(|&x| x as u64).sum();
        let weighted: u64 = sa.iter().enumerate().map(|(k, &x)| k as u64 * x as u64).sum();
        b += n * a + 16 * sum_b - weighted;
        a += sum_a;
        for &x in blocks.remainder() {
            a += x as u64;
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    ((b << 16) | a) as u32
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// Deflate length codes 257..285 (RFC 1951 3.2.5): base length and extra bits.
const LEN_BASE: [(u16, u8); 29] = [
    (3, 0),
    (4, 0),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 0),
    (9, 0),
    (10, 0),
    (11, 1),
    (13, 1),
    (15, 1),
    (17, 1),
    (19, 2),
    (23, 2),
    (27, 2),
    (31, 2),
    (35, 3),
    (43, 3),
    (51, 3),
    (59, 3),
    (67, 4),
    (83, 4),
    (99, 4),
    (115, 4),
    (131, 5),
    (163, 5),
    (195, 5),
    (227, 5),
    (258, 0),
];
/// Deflate distance codes 0..29: base distance and extra bits.
const DIST_BASE: [(u16, u8); 30] = [
    (1, 0),
    (2, 0),
    (3, 0),
    (4, 0),
    (5, 1),
    (7, 1),
    (9, 2),
    (13, 2),
    (17, 3),
    (25, 3),
    (33, 4),
    (49, 4),
    (65, 5),
    (97, 5),
    (129, 6),
    (193, 6),
    (257, 7),
    (385, 7),
    (513, 8),
    (769, 8),
    (1025, 9),
    (1537, 9),
    (2049, 10),
    (3073, 10),
    (4097, 11),
    (6145, 11),
    (8193, 12),
    (12289, 12),
    (16385, 13),
    (24577, 13),
];

/// Bit writer of a deflate stream: values LSB first, Huffman codes MSB first.
struct BitOut {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl BitOut {
    /// At most 32 bits at a time; whole bytes go out once 32 have gathered.
    fn bits(&mut self, v: u32, n: u32) {
        self.acc |= (v as u64) << self.n;
        self.n += n;
        if self.n >= 32 {
            self.out.extend_from_slice(&(self.acc as u32).to_le_bytes());
            self.acc >>= 32;
            self.n -= 32;
        }
    }
    /// A Huffman code of `n` bits, first bit = most significant.
    fn code(&mut self, c: u32, n: u32) {
        self.bits(c.reverse_bits() >> (32 - n), n);
    }
    /// Literal/length symbol of the fixed code (RFC 1951 3.2.6).
    fn symbol(&mut self, s: u32) {
        match s {
            0..=143 => self.code(0x30 + s, 8),
            144..=255 => self.code(0x190 + s - 144, 9),
            256..=279 => self.code(s - 256, 7),
            _ => self.code(0xc0 + s - 280, 8),
        }
    }
    /// Length symbol and extra bits of a match of 3..=258 bytes, then the 5-bit distance code and its extra bits.
    fn matched(&mut self, len: usize, dist_code: usize, dist_extra: u32) {
        let i = LEN_CODE[len] as usize;
        self.symbol(257 + i as u32);
        self.bits((len - LEN_BASE[i].0 as usize) as u32, LEN_BASE[i].1 as u32);
        self.code(dist_code as u32, 5);
        self.bits(dist_extra, DIST_BASE[dist_code].1 as u32);
    }
    fn finish(mut self) -> Vec<u8> {
        while self.n > 0 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n = self.n.saturating_sub(8);
        }
        self.out
    }
}

/// Index into `LEN_BASE` for each match length 0..=258 (0..2 unused).
const LEN_CODE: [u8; 259] = {
    let mut t = [0u8; 259];
    let mut len = 3;
    while len <= 258 {
        let mut i = 28;
        while LEN_BASE[i].0 as usize > len {
            i -= 1;
        }
        t[len] = i as u8;
        len += 1;
    }
    t
};

/// Number of equal bytes at `i` and `i - d`, at most 258 (the longest deflate match); eight bytes per step.
#[inline]
fn match_len(raw: &[u8], i: usize, d: usize) -> usize {
    let max = (raw.len() - i).min(258);
    let (a, b) = (&raw[i..i + max], &raw[i - d..i - d + max]);
    let mut n = 0;
    while n + 8 <= max {
        let x =
            u64::from_le_bytes(a[n..n + 8].try_into().unwrap()) ^ u64::from_le_bytes(b[n..n + 8].try_into().unwrap());
        if x != 0 {
            return n + (x.trailing_zeros() / 8) as usize;
        }
        n += 8;
    }
    while n < max && a[n] == b[n] {
        n += 1;
    }
    n
}

/// Deflate (one block, fixed Huffman codes) of picture rows: besides literals only matches against the pixel to the left
/// (distance 3) and the pixel above (distance `stride`). A scope screen is flat areas, grid lines and a few traces, so
/// that is most of what a full LZ77 finds, in one pass, with no hash table and little time on the scope's CPU. The
/// left match costs 10 bits less than the one above (no extra distance bits), so it is looked for first and the one
/// above only when the left one is not the longest possible.
fn deflate_rows(raw: &[u8], stride: usize) -> Vec<u8> {
    let (stride_code, stride_extra) = {
        let c = DIST_BASE.iter().rposition(|&(b, _)| b as usize <= stride).unwrap_or(0);
        (c, stride.saturating_sub(DIST_BASE[c].0 as usize) as u32)
    };
    let stride_ok = stride > 3 && stride <= 32768;
    let mut o = BitOut { out: Vec::with_capacity(raw.len() / 8 + 64), acc: 0, n: 0 };
    o.bits(1, 1); // BFINAL
    o.bits(1, 2); // BTYPE 01 = fixed Huffman
    let mut i = 0;
    while i < raw.len() {
        let left = if i >= 3 { match_len(raw, i, 3) } else { 0 };
        let above = if left < 258 && stride_ok && i >= stride { match_len(raw, i, stride) } else { 0 };
        if above > left && above >= 3 {
            o.matched(above, stride_code, stride_extra);
            i += above;
        } else if left >= 3 {
            o.matched(left, 2, 0);
            i += left;
        } else {
            o.symbol(raw[i] as u32);
            i += 1;
        }
    }
    o.symbol(256);
    o.finish()
}

/// PNG (8-bit RGB) of XRGB8888 pixels: filter 0 per row, zlib stream of one fixed-Huffman deflate block
/// (`deflate_rows`), so a screen of flat colour and thin lines takes a few percent of its bitmap.
pub fn png(w: u32, h: u32, px: &[u32]) -> Vec<u8> {
    let stride = w as usize * 3 + 1;
    let mut raw = vec![0u8; stride * h as usize];
    for (out, row) in raw.chunks_exact_mut(stride).zip(px.chunks(w as usize)) {
        // out[0] = 0: filter type None
        for (o, &p) in out[1..].chunks_exact_mut(3).zip(row) {
            o.copy_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
        }
    }
    let mut z = vec![0x78, 0x01]; // deflate, 32 K window, no dictionary, (0x7801 % 31 == 0)
    z.extend_from_slice(&deflate_rows(&raw, stride));
    z.extend_from_slice(&adler32(&raw).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8 bit, RGB, deflate, filter set 0, no interlace
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// PNG of the screen canvas.
/// Draws a binary PPM (P6, maxval 255) centred into the canvas (black around it); returns its size.
pub fn ppm_into(b: &[u8], cv: &mut Canvas) -> Option<(i32, i32)> {
    // header: "P6" w h maxval, separated by whitespace, comments with '#'
    let mut fields = Vec::new();
    let mut p = 2;
    if !b.starts_with(b"P6") {
        return None;
    }
    while fields.len() < 3 {
        while p < b.len() && (b[p].is_ascii_whitespace() || b[p] == b'#') {
            if b[p] == b'#' {
                while p < b.len() && b[p] != b'\n' {
                    p += 1;
                }
            }
            p += 1;
        }
        let s = p;
        while p < b.len() && b[p].is_ascii_digit() {
            p += 1;
        }
        fields.push(std::str::from_utf8(&b[s..p]).ok()?.parse::<i32>().ok()?);
    }
    let (w, h, max) = (fields[0], fields[1], fields[2]);
    p += 1;
    if max != 255 || w <= 0 || h <= 0 || b.len() < p + (w * h * 3) as usize {
        return None;
    }
    cv.fill(crate::gfx::Rect::new(0, 0, cv.w, cv.h), 0);
    let (ox, oy) = ((cv.w - w) / 2, (cv.h - h) / 2);
    for y in 0..h {
        for x in 0..w {
            let (cx, cy) = (ox + x, oy + y);
            if (0..cv.w).contains(&cx) && (0..cv.h).contains(&cy) {
                let k = p + ((y * w + x) * 3) as usize;
                cv.px[(cy * cv.w + cx) as usize] = crate::gfx::rgb(b[k] as u32, b[k + 1] as u32, b[k + 2] as u32);
            }
        }
    }
    Some((w, h))
}

pub fn png_of(cv: &Canvas) -> Vec<u8> {
    png(cv.w as u32, cv.h as u32, &cv.px)
}

/// Writes everything cached to the disks (a stick can be pulled, the scope switched off at the mains).
pub fn sync_disks() {
    // SAFETY: sync has no pointer arguments or memory preconditions.
    unsafe { libc::sync() };
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Manual picture suites must name their output directory explicitly.
    pub(crate) fn test_output_dir(key: &str) -> String {
        let dir = std::env::var(key).unwrap_or_else(|_| panic!("set {}", key));
        assert!(!dir.trim().is_empty(), "set {} to a nonempty directory", key);
        std::fs::create_dir_all(&dir).expect("create screenshot output directory");
        dir
    }
    use crate::gfx::rgb;
    use crate::math::{MathOp, Window};

    #[test]
    fn a_save_does_not_follow_links_or_write_to_devices() {
        let d = std::env::temp_dir().join(format!("oos-link-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let (victim, link) = (d.join("victim"), d.join("shot-001.png"));
        std::fs::write(&victim, b"keep").unwrap();
        std::os::unix::fs::symlink(&victim, &link).unwrap();
        assert!(write(&link, b"x").is_err());
        assert_eq!(std::fs::read(&victim).unwrap(), b"keep");
        // taken, also when it dangles
        std::fs::remove_file(&victim).unwrap();
        assert_eq!(next_name(&d, "shot", "png"), d.join("shot-002.png"));
        assert!(write(&link, b"x").is_err());
        assert!(!victim.exists());
        assert!(write(Path::new("/dev/null"), b"x").is_err());
        write(&d.join("ok.txt"), b"1").unwrap();
        write(&d.join("ok.txt"), b"2").unwrap();
        assert_eq!(std::fs::read(d.join("ok.txt")).unwrap(), b"2");
        let _ = std::fs::remove_dir_all(&d);
    }

    // Break it catches: the hardware average switch lost in a setup or on by default, the count not stored with it.
    #[test]
    fn average_settings_round_trip() {
        let mut m = Model::default();
        assert!(!m.avg_hw);
        (m.acq, m.avg_count, m.avg_hw) = (AcqMode::Average, 64, true);
        let back = parse_setup(&setup_text(&m)).unwrap();
        assert_eq!((back.acq, back.avg_count, back.avg_hw), (AcqMode::Average, 64, true));
        // older setups without the key: software average
        assert!(!parse_setup("acq = Average\navg_count = 8\n").unwrap().avg_hw);
        assert!(parse_setup("avg_hw = maybe\n").is_err());
    }

    // Break it catches: hardware segment settings lost in a setup, a count clamped to the software limit, a negative
    // or absurd interval reaching 0x4068.
    #[test]
    fn segment_settings_round_trip() {
        let mut m = Model::default();
        (m.segments, m.seg_hw, m.seg_ivl) = (20_000, true, 1e-3);
        let back = parse_setup(&setup_text(&m)).unwrap();
        assert_eq!((back.segments, back.seg_hw, back.seg_ivl), (20_000, true, 1e-3));
        let d = parse_setup("segments = 900000\nseg_interval = -1\n").unwrap();
        assert_eq!((d.segments, d.seg_hw, d.seg_ivl), (crate::ui::SEG_HW_MAX, false, 0.0));
        assert_eq!(parse_setup("seg_interval = 1e9\n").unwrap().seg_ivl, crate::ui::SEG_IVL_MAX);
    }

    #[test]
    fn tdiv_by_value_and_legacy_index() {
        let m = parse_setup("tdiv = 1e-9\nzoom_tdiv = 1e-9\n").unwrap();
        assert_eq!((tscales()[m.tscale], tscales()[m.zscale]), (1e-9, 1e-9));
        assert!(setup_text(&m).contains("tdiv = 0.000000001"));
        // old ladder started at 5 ns: index 0 was 5 ns/div, 3 was 50 ns
        let m = parse_setup("tscale = 3\nzscale = 0\n").unwrap();
        assert_eq!((tscales()[m.tscale], tscales()[m.zscale]), (50e-9, 5e-9));
        assert!(parse_setup("tdiv = 3e-9").is_err());
    }

    #[test]
    fn vdiv_by_value_and_legacy_index() {
        let m = parse_setup("ch1.vdiv = 0.0001\nch2.vscale = 10\n").unwrap();
        assert_eq!(VSCALES[m.ch[0].vscale], 100e-6);
        // old ladder started at 500 uV: index 10 was 1 V/div
        assert_eq!(VSCALES[m.ch[1].vscale], 1.0);
        assert_eq!(VSCALES[Model::default().ch[0].vscale], 1.0);
        assert!(setup_text(&m).contains("ch1.vdiv = 0.0001"));
    }

    /// A non-default set-up in every group of fields.
    pub(super) fn custom() -> Model {
        let mut m = Model::default();
        (m.bode_sw_time, m.bode_sw_log, m.bode_sync, m.bode_sync_slope, m.bode_sync_level, m.bode_sw_in, m.bode_ref) =
            (12.5, false, 2, Slope::Fall, -0.75, 0, -6.02);
        m.ch[1].on = true;
        m.ch[1].vscale = 1; // 200 uV/div
        m.ch[1].offset = -0.123_456_79;
        m.ch[2].coupling = Coupling::Ac;
        m.ch[2].bw = BW_20M;
        m.ch[1].bw = 17;
        m.ch[3].probe = 10.0;
        m.ch[3].invert = true;
        m.active = 2;
        m.tscale = 5;
        m.delay = 1.5e-7;
        m.trig_src = 3;
        m.trig_level = 0.1;
        m.slope = Slope::Either;
        m.trig_mode = TrigMode::Normal;
        m.trig_type = TrigType::SetupHold;
        m.trig_level2 = -2.25;
        m.trig_src2 = 1;
        m.trig_slope2 = Slope::Fall;
        m.trig_pol = Pol::Neg;
        m.trig_when = WhenKind::Out;
        m.trig_t1 = 3e-9;
        m.trig_t2 = 0.7;
        m.video_std = VideoStd::Ntsc;
        m.video_sync = VSync::Line;
        m.video_line = 123;
        m.pat = [Bit::R, Bit::F, Bit::L, Bit::X];
        m.pat_levels = [0.1, -0.2, 3.3, 1e-3];
        m.win_mode = WinMode::Inside;
        m.nth_idle = 2e-3;
        m.nth_n = 7;
        m.setup = 5e-9;
        m.hold = 6e-9;
        m.acq = AcqMode::Average;
        m.avg_count = 256;
        m.mdepth = 4;
        m.persist = 6;
        m.intensity = 85;
        m.grid = Grid::Light;
        m.meas = vec![(Meas::Rise, WaveId::Ch(3)), (Meas::Vrms, WaveId::Ch(0)), (Meas::PDuty, WaveId::Ch(1))];
        m.strip_dock = StripDock::Right;
        m.cursor = CursorMode::Volt;
        m.cur_a = 0.125;
        m.cur_b = 0.875;
        m.touch_lock = true;
        m.aux_out = AuxOut::Off;
        m.ser = SerialTrig {
            rel: Rel::Out,
            max: 0x1_2345_6789,
            mask: 0xf0f0,
            uart_inv: true,
            uart_stop: StopBits::Two,
            uart_err: UartErr::Parity,
            spi_cs_high: true,
            spi_timeout: 2e-6,
            addr_rel: Rel::In,
            addr_max: 0x3a0,
            dir: I2cDir::Read,
            ten_bit: true,
            bytes: 4,
        };
        m.trig_bus_val = 0xab_cdef_0123;
        m.maths[3] = crate::math::MathCh {
            op: MathOp::Bpf,
            a: 4,
            b: 0,
            scale: 7,
            fc: 1234.0,
            bw: 56.0,
            offset: -1.5,
            win: Window::Blackman,
            lin: (1.0, 0.0),
        };
        m.math_bw = 777.0;
        m.zones[1] = crate::zone::Zone {
            on: true,
            src: 2,
            inside: false,
            pts: vec![(-1e-4, 0.5), (2.5e-5, 1.25), (3e-5, -0.75)],
        };
        m.dots = DotsMode::Off;
        m.z_src = Some(3);
        m.hist = Hist::Horizontal;
        (
            m.view,
            m.spec_src,
            m.spec_center,
            m.spec_span,
            m.spec_rbw,
            m.spec_trace,
            m.spec_ref,
            m.spec_wf,
            m.spec_dist,
        ) = (AView::Spectrum, WaveId::Ch(2), 1.5e6, 2e5, 1e3, 2, -12.5, true, true);
        (m.spec_sort, m.spec_harm, m.trend_sel) = (1, 4, [3, 1]);
        (m.eye_src, m.eye_rate, m.pw_v, m.pw_i, m.pw_mode) =
            (WaveId::Ch(3), 1.25e9, WaveId::Ch(2), WaveId::Ch(0), PowerMode::Switching);
        m.math = MathOp::Fft;
        m.math_a = 2;
        m.math_b = 3;
        m.math_scale = 2;
        m.math_fc = 12345.0;
        m.math_offset = -1.3;
        m.fft_win = Window::Flattop;
        m.dec_on = true;
        m.dec_src = 1;
        m.dec_baud = 3;
        m.dec_thr = 0.8;
        m.dec_ascii = false;
        m.dec_proto = Proto::SPI;
        (m.dec_arate, m.dec_tick, m.dec_nibbles, m.dec_uspeed) = (2, 3, 4, 2);
        m.dec_src2 = 0;
        m.dec_cs = None;
        m.dec_mode = 3;
        m.dec_bits = 16;
        m.dec_parity = Parity::Odd;
        m.zoom = true;
        m.zscale = 2;
        m.zdelay = -3e-8;
        m.color_grade = true;
        m.mask_on = true;
        m.mask_src = 3;
        m.mask_x = 0.5;
        m.mask_y = 1.25;
        m.mask_stop = true;
        m.quick = Quick::SaveCsv;
        m.la = crate::ui::la::LaSet {
            on: true,
            show: 0x0f0f,
            preset: [2, crate::ui::la::USER],
            user: [0.5, -0.25],
            height: 2,
        };
        m
    }

    #[test]
    fn a_ref_in_a_setup_stays_a_ref_and_a_channel_number_stays_a_channel() {
        let mut m = Model::default();
        m.mag_v = WaveId::Ref(1);
        m.mag_i = WaveId::Ch(0);
        m.dpt_v = WaveId::Ref(0);
        m.inv_a = WaveId::Ref(4);
        m.inv_tq = Some(WaveId::Ref(9));
        m.spec_src = WaveId::Ref(0);
        m.eye_src = WaveId::Ref(2);
        m.pw_v = WaveId::Ref(1);
        m.pw_i = WaveId::Ch(0);
        m.bode_in = WaveId::Ref(3);
        m.bode_out = WaveId::Ch(1);
        m.meas = vec![(Meas::Vpp, WaveId::Ref(0))];
        m.meas2 = vec![(0, WaveId::Ref(0), WaveId::Ch(1))];
        let text = setup_text(&m);
        assert!(text.contains("magnetic = R2 1 "));
        assert!(text.contains("dpt = R1 "));
        assert!(text.contains("spec = R1 "));
        assert!(text.contains("eye = R3 "));
        assert!(text.contains("power = R2 1 "));
        assert!(text.contains("bode = "));
        assert!(text.contains(" R4 2 "));
        assert!(text.contains("meas = Vpp:R1"));
        assert!(text.contains("meas2 = 0:R1:2"));
        let back = parse_setup(&text).unwrap();
        assert_eq!(
            (back.mag_v, back.mag_i, back.dpt_v, back.inv_a, back.inv_tq),
            (m.mag_v, m.mag_i, m.dpt_v, m.inv_a, m.inv_tq)
        );
        assert_eq!(
            (back.spec_src, back.eye_src, back.pw_v, back.pw_i, back.bode_in, back.bode_out),
            (m.spec_src, m.eye_src, m.pw_v, m.pw_i, m.bode_in, m.bode_out)
        );
        assert_eq!((back.meas, back.meas2), (m.meas, m.meas2));
        // a setup written before refs were sources
        let old = parse_setup("magnetic = 1 2 1 0 0 0 IPsi 0\ninverter = Dq0 1 2 3 Off 1 4 true off off off 1 1 0 false 0 1 off 1024 false 0 1 2\ndpt = 1 2 3 Off 4 10 Both\n").unwrap();
        assert_eq!(old.mag_v, WaveId::Ch(0));
        assert_eq!(old.mag_i, WaveId::Ch(1));
        assert_eq!(old.inv_a, WaveId::Ch(0));
        assert_eq!(old.dpt_g, WaveId::Ch(2));
        assert_eq!(old.inv_tq, None);
    }

    #[test]
    fn catalog_setup_validates_ids_sources_duplicates_and_shared_selection_limit() {
        let mut model = Model::default();
        set(&mut model, "cat", "90:R1:R2:3:4 170:R3:R4:R5:4").unwrap();
        let text = setup_text(&model);
        let restored = parse_setup(&text).unwrap();
        assert_eq!(restored.cat, model.cat);
        for invalid in ["999:R1:R2:3:4", "1:R1:R2:3:4", "90:R99:R2:3:4", "90:R1:R2:3", "90:R1:R2:3:4 90:R1:R2:3:4"] {
            let before = model.cat.clone();
            assert!(set(&mut model, "cat", invalid).is_err(), "{invalid}");
            assert_eq!(model.cat, before);
        }
        model.meas = vec![(Meas::Vpp, WaveId::Ch(0)); MEAS_SELECTION_MAX - 1];
        model.meas2.clear();
        model.cat.clear();
        assert!(set(&mut model, "cat", "90:R1:R2:3:4 170:R3:R4:R5:4").is_err());
        assert!(model.cat.is_empty());
        set(&mut model, "cat", "90:R1:R2:3:4").unwrap();
        assert!(set(&mut model, "meas2", "0:1:2").is_err());
        assert!(model.meas2.is_empty());
        set(&mut model, "cat", "").unwrap();
        assert!(model.cat.is_empty());
    }

    #[test]
    fn setup_round_trip() {
        let m = custom();
        assert_ne!(m, Model::default());
        let text = setup_text(&m);
        let back = parse_setup(&text).unwrap();
        assert_eq!(back, m);
        // and the default set-up
        assert_eq!(parse_setup(&setup_text(&Model::default())).unwrap(), Model::default());
    }

    #[test]
    fn setup_sim_advanced_round_trip_and_legacy_defaults() {
        let mut model = Model::default();
        model.sim_prbs = 13;
        model.sim_ssc_center = true;
        model.sim_ssc_deviation = 0.01;
        model.sim_ssc_hz = 40e3;
        model.sim_dcd_pp = 8e-12;
        model.sim_corr_rj = 2e-12;
        model.sim_corr_hz = 2e6;
        let setup = setup_text(&model);
        assert_eq!(parse_setup(&setup).unwrap(), model);
        let legacy = setup.lines().filter(|line| !line.starts_with("sim_extra =")).collect::<Vec<_>>().join("\n");
        let old = parse_setup(&legacy).unwrap();
        assert_eq!(old.sim_prbs, 13);
        assert!(!old.sim_ssc_center);
        assert_eq!(old.sim_ssc_deviation, 0.005);
        assert_eq!(old.sim_ssc_hz, 33e3);
        assert_eq!(old.sim_dcd_pp, 0.0);
        assert_eq!(old.sim_corr_rj, 0.0);
        assert_eq!(old.sim_corr_hz, 1e6);
        for invalid in
            ["true NaN 33000 0 0 1e6", "false .005 0 0 0 1e6", "true .005 33000 -1 0 1e6", "false .005 33000 0 1 1e6"]
        {
            assert!(parse_setup(&format!("sim_extra = {invalid}\n")).is_err());
        }
    }

    #[test]
    fn setup_errors_and_unknown_keys() {
        let m = parse_setup("# comment\n\nnew_key_of_a_later_version = 3\nch2.on = true\n").unwrap();
        assert!(m.ch[1].on);
        assert!(parse_setup("tscale = 9999").unwrap_err().contains("line 1: tscale"));
        assert!(parse_setup("ch1.coupling = XY").is_err());
        assert!(parse_setup("ch9.on = true").is_err());
        assert!(parse_setup("pat = H L").is_err());
        assert!(parse_setup("delay").is_err());
    }

    /// AUX OUT: written by name, missing key = trigger out (the FPGA's power-up state).
    #[test]
    fn setup_dots_by_name_and_legacy_bool() {
        assert!(setup_text(&Model::default()).contains("dots = Auto\n"));
        for &d in DOTS_MODES.iter() {
            assert_eq!(parse_setup(&format!("dots = {:?}", d)).unwrap().dots, d);
        }
        // saved before Auto existed (true was the old default, not a choice): both load as Auto
        assert_eq!(parse_setup("dots = true").unwrap().dots, DotsMode::Auto);
        assert_eq!(parse_setup("dots = false").unwrap().dots, DotsMode::Auto);
        assert!(parse_setup("dots = sometimes").is_err());
    }

    #[test]
    fn setup_aux_out() {
        assert_eq!(parse_setup("ch2.on = true").unwrap().aux_out, AuxOut::TrigOut);
        assert!(setup_text(&Model::default()).contains("aux_out = TrigOut\n"));
        assert_eq!(parse_setup("aux_out = Off").unwrap().aux_out, AuxOut::Off);
        assert!(parse_setup("aux_out = Blink").is_err());
        assert_eq!(parse_setup("aux_out = PassFail").unwrap().aux_out, AuxOut::PassFail);
    }

    /// The Quick key action: written by name, read back, unknown names rejected, missing key = Screenshot.
    #[test]
    fn setup_quick_key() {
        for &q in QUICKS.iter() {
            let mut m = Model::default();
            m.quick = q;
            let text = setup_text(&m);
            assert!(text.contains(&format!("quick = {:?}\n", q)));
            assert_eq!(parse_setup(&text).unwrap().quick, q);
        }
        assert_eq!(parse_setup("quick = Autoset\nquick_v2 = Whatever\n").unwrap().quick, Quick::Autoset);
        assert_eq!(parse_setup("ch1.on = true\n").unwrap().quick, Quick::Screenshot);
        assert!(parse_setup("quick = Coffee").unwrap_err().contains("line 1: quick"));
    }

    /// Every protocol is stored by name; the USB decoder with its D- source and speed.
    #[test]
    #[cfg_attr(not(feature = "decoders"), ignore = "needs every decoder")]
    fn setup_protocols_by_name() {
        for &p in PROTOS.iter() {
            let mut m = Model::default();
            m.dec_proto = p;
            let text = setup_text(&m);
            assert!(text.contains(&format!("dec_proto = {}\n", p.key())));
            assert_eq!(parse_setup(&text).unwrap().dec_proto, p);
        }
        // the protocols that became modules keep the names of setups saved before
        for (old, name) in [("Dmx512", "DMX512"), ("Spdif", "S/PDIF"), ("UsbPd", "USB PD")] {
            assert_eq!(parse_setup(&format!("dec_proto = {}\n", old)).unwrap().dec_proto.name(), name);
        }
        // a module's options go with it
        let mut m = Model::default();
        m.dec_proto = Proto::from_key("Example").unwrap();
        m.dec_opt = [2, 1, 0, 0];
        let text = setup_text(&m);
        assert!(text.contains("dec_proto = Example\n") && text.contains("dec_opt = 2 1 0 0\n"));
        assert_eq!(parse_setup(&text).unwrap(), m);
        assert!(parse_setup("dec_proto = NoSuchBus").is_err());
        let mut m = Model::default();
        (m.dec_proto, m.dec_src, m.dec_cs, m.dec_uspeed) = (Proto::from_key("Usb").unwrap(), 2, Some(3), 1);
        let text = setup_text(&m);
        assert!(text.contains("dec_proto = Usb\n") && text.contains("dec_uspeed = 1\n"));
        assert_eq!(parse_setup(&text).unwrap(), m);
        assert!(parse_setup("dec_uspeed = 3").is_err());
    }

    #[test]
    fn csv_of_a_record() {
        let mut m = Model::default();
        m.ch[2].on = true;
        let mut rec = Record {
            t0: -1e-3,
            dt: 1e-6,
            ch: Default::default(),
            triggered: true,
            partial: false,
            raw: None,
            la: None,
            pairs: false,
        };
        rec.ch[0] = vec![0.5, 1.25, f32::NAN];
        rec.ch[2] = vec![-1.0, 0.0, 3.3];
        let csv = wave_csv(&rec, &m);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "Time (s),CH1 (V),CH3 (V)");
        assert_eq!(lines[1], "-1.000000000e-3,0.5,-1");
        assert_eq!(lines[3], "-9.980000000e-4,,3.3");
        assert_eq!(lines.len(), 4);
        let back = parse_wave(csv.as_bytes(), "wave.csv").unwrap();
        assert_eq!(back.len(), 2);
        assert!((back[0].dt - 1e-6).abs() < 1e-18);
        assert_eq!(back[0].v[0], 0.5);
        assert_eq!(back[1].v[2], 3.3);
    }

    /// A line wider than the reference slots ends the parse: before, one line of a million columns made a million
    /// traces (40 MB of text took 1.9 GB) before the slots refused them.
    #[test]
    fn csv_wider_than_the_reference_slots_is_refused_while_parsing() {
        let wide = format!("0{}\n1,0\n", ",0".repeat(NREF + 1));
        assert!(parse_wave(wide.as_bytes(), "wide.csv").unwrap_err().contains("more traces"));
        let full = format!("0{}\n1{}\n", ",1".repeat(NREF), ",2".repeat(NREF));
        assert_eq!(parse_wave(full.as_bytes(), "full.csv").unwrap().len(), NREF);
        // ragged lines: a column missing on a line, or starting late, is NaN there
        let back = parse_wave(b"Time,A\n0,1\n1,2,7\n2,,8\n3\n", "r.csv").unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!((back[0].t0, back[0].dt, back[0].label.as_str()), (0.0, 1.0, "CH1"));
        assert_eq!(&back[0].v[..2], [1.0, 2.0]);
        assert!(back[0].v[2].is_nan() && back[0].v.len() == 3);
        assert!(back[1].v[0].is_nan());
        assert_eq!(&back[1].v[1..], [7.0, 8.0]);
    }

    #[test]
    fn oos_bin_and_wfm_round_trip() {
        let mut m = Model::default();
        m.ch[0].on = true;
        let rec = Record {
            t0: -2e-6,
            dt: 5e-9,
            ch: {
                let mut c: [Vec<f32>; NCH] = Default::default();
                c[0] = vec![0.1, 0.2, -0.3, 0.4];
                c
            },
            triggered: true,
            partial: false,
            raw: None,
            la: None,
            pairs: false,
        };
        let bin = wave_bin(&rec, &m);
        let t = &parse_wave(&bin, "wave.bin").unwrap()[0];
        assert_eq!(t.v, rec.ch[0]);
        assert!((t.dt - rec.dt).abs() < 1e-18);
        assert!((t.t0 - rec.t0).abs() < 1e-18);

        let v = [0.0_f32, 0.5, 1.0, -0.25];
        let wfm = wfm_f32(b":WFM#003", -1e-6, 1e-9, &v, "lane");
        let g = parse_wave(&wfm, "x.wfm").unwrap();
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].label, "lane");
        assert!((g[0].dt - 1e-9).abs() < 1e-18);
        assert!((g[0].t0 + 1e-6).abs() < 1e-18);
        for (a, b) in g[0].v.iter().zip(v) {
            assert!((a - b).abs() < 1e-6, "{a} {b}");
        }
        let old = wfm_f32(b":WFM#001", -1e-6, 1e-9, &v, "lane");
        assert_eq!(parse_wave(&old, "old.wfm").unwrap()[0].v, v);
        let two = wfm_f32(b":WFM#002", 0.0, 2e-9, &v, "two");
        assert_eq!(parse_wave(&two, "two.wfm").unwrap()[0].label, "two");
    }

    /// A short Tek WFM curve of f32 samples, little endian.
    fn wfm_f32(magic: &[u8; 8], t0: f64, dt: f64, v: &[f32], label: &str) -> Vec<u8> {
        let (explicit, implicit, specification, curve) =
            if magic == b":WFM#001" { (166, 478, 790, 820) } else { (168, 488, 808, 838) };
        let mut b = vec![0u8; curve + v.len() * 4 + 8];
        b[0..2].copy_from_slice(&0x0F0Fu16.to_le_bytes());
        b[2..10].copy_from_slice(magic);
        b[15] = 4;
        b[16..20].copy_from_slice(&(curve as i32).to_le_bytes());
        let lab = label.as_bytes();
        b[40..40 + lab.len()].copy_from_slice(lab);
        let put_f64 = |b: &mut [u8], o: usize, x: f64| b[o..o + 8].copy_from_slice(&x.to_le_bytes());
        let put_u32 = |b: &mut [u8], o: usize, x: u32| b[o..o + 4].copy_from_slice(&x.to_le_bytes());
        put_f64(&mut b, explicit, 1.0);
        put_f64(&mut b, explicit + 8, 0.0);
        put_u32(&mut b, explicit + 72, 4);
        put_f64(&mut b, implicit, dt);
        put_f64(&mut b, implicit + 8, t0);
        put_u32(&mut b, specification + 14, 0);
        put_u32(&mut b, specification + 18, (v.len() * 4) as u32);
        for (k, &x) in v.iter().enumerate() {
            b[curve + k * 4..curve + k * 4 + 4].copy_from_slice(&x.to_le_bytes());
        }
        b
    }

    #[test]
    fn all_tek_wfm_versions_keep_voltage_calibration_time_origin_and_byte_order() {
        for magic in [b":WFM#001", b":WFM#002", b":WFM#003"] {
            let (explicit, implicit, specification, curve) =
                if magic == b":WFM#001" { (166, 478, 790, 820) } else { (168, 488, 808, 838) };
            for big_endian in [false, true] {
                let codes = [-2.0, 0.0, 2.0, 4.0];
                let mut bytes = wfm_f32(magic, -9.5e-7, 20e-12, &codes, "captured");
                bytes[explicit..explicit + 8].copy_from_slice(&0.125f64.to_le_bytes());
                bytes[explicit + 8..explicit + 16].copy_from_slice(&0.75f64.to_le_bytes());
                if big_endian {
                    bytes[0..2].copy_from_slice(&[0xf0, 0xf0]);
                    for offset in [16, explicit + 72, specification + 14, specification + 18] {
                        bytes[offset..offset + 4].reverse();
                    }
                    for offset in [explicit, explicit + 8, implicit, implicit + 8] {
                        bytes[offset..offset + 8].reverse();
                    }
                    for offset in (curve..curve + codes.len() * 4).step_by(4) {
                        bytes[offset..offset + 4].reverse();
                    }
                }
                let parsed = parse_wave(&bytes, "capture.wfm").unwrap();
                assert_eq!(parsed[0].v, [0.5, 0.75, 1.0, 1.25]);
                assert_eq!(parsed[0].label, "captured");
                assert_eq!(parsed[0].dt, 20e-12);
                assert_eq!(parsed[0].t0, -9.5e-7);
            }
        }
    }

    #[test]
    fn tek_wfm_rejects_truncated_misaligned_or_invalid_scaling_before_decoding() {
        let original = wfm_f32(b":WFM#001", 0.0, 20e-12, &[-1.0, 1.0], "capture");
        let mut truncated = original.clone();
        truncated.remove(truncated.len() - 9);
        assert!(parse_wave(&truncated, "capture.wfm").unwrap_err().contains("truncated"));
        let mut wrong_size = original.clone();
        wrong_size[15] = 2;
        assert!(parse_wave(&wrong_size, "capture.wfm").unwrap_err().contains("bytes per point"));
        let mut negative = original.clone();
        negative[16..20].copy_from_slice(&(-1i32).to_le_bytes());
        assert!(parse_wave(&negative, "capture.wfm").unwrap_err().contains("negative"));
        let mut bad_time = original.clone();
        bad_time[478..486].copy_from_slice(&f64::NAN.to_le_bytes());
        assert!(parse_wave(&bad_time, "capture.wfm").unwrap_err().contains("finite"));
        let mut bad_voltage = original;
        bad_voltage[166..174].copy_from_slice(&f64::INFINITY.to_le_bytes());
        assert!(parse_wave(&bad_voltage, "capture.wfm").unwrap_err().contains("finite"));
    }

    #[test]
    fn wave_and_fast_frame_round_trip() {
        let mut m = Model::default();
        m.ch[0].on = true;
        m.ch[2].on = true;
        let mut rec = Record {
            t0: -1e-3,
            dt: 1e-6,
            ch: Default::default(),
            triggered: true,
            partial: false,
            raw: None,
            la: None,
            pairs: false,
        };
        rec.ch[0] = vec![0.5, 1.25];
        rec.ch[2] = vec![-1.0, 0.0];
        let b = wave_bin(&rec, &m);
        let (back, n) = parse_wave_bin(&b).unwrap();
        assert_eq!(n, b.len());
        assert_eq!(back.dt, rec.dt);
        assert_eq!(back.ch[0], rec.ch[0]);
        assert_eq!(back.ch[2], rec.ch[2]);
        let ff = ff_bin(&[rec.clone(), rec.clone()], &m);
        let recs = parse_ff(&ff).unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[1].ch[0], rec.ch[0]);
        assert_eq!(parse_ff(&b).unwrap().len(), 1);
        let t = mask_text(2, 0.2, 0.4, true, &[1.0, 2.5]);
        let mf = parse_mask(&t).unwrap();
        assert_eq!((mf.src, mf.inv, mf.v), (Some(2), Some(true), vec![1.0, 2.5]));
        let old = parse_mask("1.0\n2.0\n").unwrap();
        assert_eq!(old.v, vec![1.0, 2.0]);
        assert!(old.src.is_none());
    }

    /// Counts in a frame file come from the file. One that does not fit the bytes is an error: before, a sample count
    /// near 2^62 overflowed the offset (a panic, and with panic=abort the end of oos-ui) and a frame count of 2^32 - 1
    /// asked for a list of that many records.
    #[test]
    fn frame_file_counts_beyond_the_bytes_are_refused() {
        let mut huge = b"OOSW".to_vec();
        for x in [1u32, 1] {
            huge.extend_from_slice(&x.to_le_bytes());
        }
        huge.extend_from_slice(&((1u64 << 62) + 1).to_le_bytes());
        huge.extend_from_slice(&1e-9f64.to_le_bytes());
        huge.extend_from_slice(&0f64.to_le_bytes());
        huge.extend_from_slice(&1u32.to_le_bytes());
        huge.extend_from_slice(&[0u8; 16]);
        assert!(parse_wave_bin(&huge).err().unwrap().contains("limit"));
        let over = (MAX_WAVEFORM_SAMPLES as u64 + 1).to_le_bytes();
        huge[12..20].copy_from_slice(&over);
        assert!(parse_ff(&huge).err().unwrap().contains("limit"));
        huge[12..20].copy_from_slice(&5u64.to_le_bytes());
        assert!(parse_wave_bin(&huge).err().unwrap().contains("truncated"));
        huge[12..20].copy_from_slice(&4u64.to_le_bytes());
        assert_eq!(parse_wave_bin(&huge).unwrap().1, huge.len());

        let mut ff = b"OOSF".to_vec();
        ff.extend_from_slice(&1u32.to_le_bytes());
        ff.extend_from_slice(&u32::MAX.to_le_bytes());
        ff.extend_from_slice(&huge);
        assert!(parse_ff(&ff).err().unwrap().contains("truncated"));
        ff[8..12].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(parse_ff(&ff).unwrap().len(), 1);
    }

    #[test]
    fn checksums() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        // the eight-byte steps against the plain bitwise definitions, all lengths around the step sizes
        let data: Vec<u8> = (0..20000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
        for n in (0..70).chain([5551, 5552, 5553, 11104, 20000]) {
            let d = &data[..n];
            let crc = !d.iter().fold(!0u32, |mut c, &x| {
                c ^= x as u32;
                for _ in 0..8 {
                    c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
                }
                c
            });
            assert_eq!(crc32(d), crc, "CRC of {} bytes", n);
            let (a, b) = d
                .iter()
                .fold((1u32, 0u32), |(a, b), &x| ((a + x as u32) % 65521, (b + (a + x as u32) % 65521) % 65521));
            assert_eq!(adler32(d), b << 16 | a, "Adler of {} bytes", n);
        }
        assert_eq!(adler32(&[0xff; 100_000]), {
            let (a, b) =
                (0..100_000).fold((1u64, 0u64), |(a, b), _| ((a + 255) % 65521, (b + (a + 255) % 65521) % 65521));
            (b << 16 | a) as u32
        });
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11e6_0398);
        assert_eq!(adler32(&[]), 1);
    }

    /// Reads a deflate stream bit by bit (RFC 1951 3.1.1): values LSB first, Huffman codes MSB first.
    struct Bits<'a> {
        d: &'a [u8],
        p: usize,
    }
    impl Bits<'_> {
        fn bit(&mut self) -> u32 {
            let b = (self.d[self.p >> 3] >> (self.p & 7)) & 1;
            self.p += 1;
            b as u32
        }
        fn bits(&mut self, n: u32) -> u32 {
            (0..n).fold(0, |v, i| v | self.bit() << i)
        }
    }

    /// One symbol of the fixed Huffman code (RFC 1951 3.2.6): 7 bits 256..279, 8 bits 0..143 and 280..287, 9 bits 144..255.
    fn fixed_symbol(b: &mut Bits) -> u32 {
        let mut c = (0..7).fold(0, |c, _| c << 1 | b.bit());
        if c <= 0b001_0111 {
            return 256 + c;
        }
        c = c << 1 | b.bit();
        if (0b0011_0000..=0b1011_1111).contains(&c) {
            return c - 0b0011_0000;
        }
        if (0b1100_0000..=0b1100_0111).contains(&c) {
            return 280 + c - 0b1100_0000;
        }
        c = c << 1 | b.bit();
        144 + c - 0b1_1001_0000
    }

    /// Independent inflate for what the encoder may write (stored and fixed Huffman blocks, not dynamic ones); returns the
    /// data and the number of bytes the stream took.
    fn inflate(z: &[u8]) -> (Vec<u8>, usize) {
        const LBASE: [u32; 29] = [
            3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195,
            227, 258,
        ];
        const LEXTRA: [u32; 29] =
            [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
        const DBASE: [u32; 30] = [
            1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073,
            4097, 6145, 8193, 12289, 16385, 24577,
        ];
        const DEXTRA: [u32; 30] =
            [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
        let mut b = Bits { d: z, p: 0 };
        let mut out: Vec<u8> = Vec::new();
        loop {
            let last = b.bit();
            match b.bits(2) {
                0 => {
                    b.p = (b.p + 7) & !7;
                    let (n, nn) = (b.bits(16), b.bits(16));
                    assert_eq!(n, !nn & 0xffff, "stored block length check");
                    for _ in 0..n {
                        out.push(b.bits(8) as u8);
                    }
                }
                1 => loop {
                    let s = fixed_symbol(&mut b);
                    if s < 256 {
                        out.push(s as u8);
                    } else if s == 256 {
                        break;
                    } else {
                        assert!(s <= 285, "length symbol {}", s);
                        let i = (s - 257) as usize;
                        let len = LBASE[i] + b.bits(LEXTRA[i]);
                        let dc = (0..5).fold(0, |c, _| c << 1 | b.bit()) as usize;
                        assert!(dc < 30, "distance code {}", dc);
                        let dist = (DBASE[dc] + b.bits(DEXTRA[dc])) as usize;
                        assert!(
                            dist <= out.len(),
                            "distance {} reaches before the start ({} bytes so far)",
                            dist,
                            out.len()
                        );
                        for _ in 0..len {
                            out.push(out[out.len() - dist]);
                        }
                    }
                },
                t => panic!("block type {} is not written by the encoder", t),
            }
            if last == 1 {
                break;
            }
        }
        (out, (b.p + 7) / 8)
    }

    #[test]
    fn inflate_reference_streams() {
        // Python zlib, Z_FIXED, raw deflate of b"hello hello hello hello": "hello " as literals, then one match (distance 6)
        let z = [0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x40, 0x27, 0x01];
        let (out, used) = inflate(&z);
        assert_eq!(out, b"hello hello hello hello");
        assert_eq!(used, z.len());
        // a stored block
        let (out, used) = inflate(&[1, 3, 0, 0xfc, 0xff, b'a', b'b', b'c']);
        assert_eq!((out.as_slice(), used), (&b"abc"[..], 8));
    }

    /// Parses the PNG structure: signature, chunk lengths and CRCs, IHDR, the zlib stream (inflated, Adler32) and the
    /// pixels.
    fn decode(png: &[u8]) -> (u32, u32, Vec<u8>) {
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let mut i = 8;
        let (mut w, mut h, mut z) = (0, 0, Vec::new());
        let mut kinds = Vec::new();
        while i < png.len() {
            let n = u32::from_be_bytes(png[i..i + 4].try_into().unwrap()) as usize;
            let kind = &png[i + 4..i + 8];
            let data = &png[i + 8..i + 8 + n];
            let crc = u32::from_be_bytes(png[i + 8 + n..i + 12 + n].try_into().unwrap());
            assert_eq!(crc, crc32(&png[i + 4..i + 8 + n]), "CRC of {:?}", std::str::from_utf8(kind));
            match kind {
                b"IHDR" => {
                    w = u32::from_be_bytes(data[0..4].try_into().unwrap());
                    h = u32::from_be_bytes(data[4..8].try_into().unwrap());
                    assert_eq!(&data[8..], &[8, 2, 0, 0, 0]);
                }
                b"IDAT" => z.extend_from_slice(data),
                _ => {}
            }
            kinds.push(String::from_utf8_lossy(kind).into_owned());
            i += 12 + n;
        }
        assert_eq!(kinds, ["IHDR", "IDAT", "IEND"]);
        assert_eq!(i, png.len());
        // zlib header, deflate data, Adler32
        assert_eq!(((z[0] as u32) << 8 | z[1] as u32) % 31, 0);
        assert_eq!(z[0] & 0x0f, 8);
        assert_eq!(z[1] & 0x20, 0, "no preset dictionary");
        let (raw, used) = inflate(&z[2..]);
        let p = 2 + used;
        assert_eq!(u32::from_be_bytes(z[p..p + 4].try_into().unwrap()), adler32(&raw));
        assert_eq!(p + 4, z.len());
        // rows: filter 0 + RGB
        let mut pix = Vec::new();
        for row in raw.chunks(w as usize * 3 + 1) {
            assert_eq!(row[0], 0);
            pix.extend_from_slice(&row[1..]);
        }
        assert_eq!(pix.len(), (w * h * 3) as usize);
        (w, h, pix)
    }

    #[test]
    fn ppm_power_on_picture() {
        let mut b = b"P6\n# OOS\n2 1\n255\n".to_vec();
        b.extend_from_slice(&[255, 0, 0, 0, 0, 255]);
        let mut cv = Canvas::new(10, 4);
        assert_eq!(ppm_into(&b, &mut cv), Some((2, 1)));
        assert_eq!(
            (cv.px[(1 * 10 + 4) as usize], cv.px[(1 * 10 + 5) as usize]),
            (crate::gfx::rgb(255, 0, 0), crate::gfx::rgb(0, 0, 255))
        );
        assert!(ppm_into(b"P5\n1 1\n255\n\0", &mut cv).is_none());
    }

    #[test]
    fn png_structure_and_pixels() {
        // larger than one stored block (65535 bytes)
        let (w, h) = (301u32, 97u32);
        let px: Vec<u32> = (0..w * h).map(|i| i.wrapping_mul(2654435761u32) & 0xff_ffff).collect();
        let data = png(w, h, &px);
        let (dw, dh, pix) = decode(&data);
        assert_eq!((dw, dh), (w, h));
        for (k, &p) in px.iter().enumerate() {
            assert_eq!(&pix[k * 3..k * 3 + 3], &[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
        }
    }

    /// A picture like the scope screen: black, a grid, filled boxes, a trace, some speckle.
    fn screen_like(w: u32, h: u32) -> Vec<u32> {
        let mut px = vec![0u32; (w * h) as usize];
        for y in 0..h {
            for x in 0..w {
                let mut c = 0;
                if x % 100 == 0 || y % 75 == 0 {
                    c = rgb(60, 60, 60);
                }
                if (40..200).contains(&x) && (30..70).contains(&y) {
                    c = rgb(20, 90, 160);
                }
                let trace = (h as f32 / 2.0 + (x as f32 / 20.0).sin() * h as f32 / 4.0) as u32;
                if y == trace || y == trace + 1 {
                    c = rgb(255, 220, 0);
                }
                if (x * 7 + y * 13) % 997 == 0 {
                    c = rgb(200, 200, 200);
                }
                px[(y * w + x) as usize] = c;
            }
        }
        px
    }

    fn assert_decodes_to(data: &[u8], w: u32, h: u32, px: &[u32]) {
        let (dw, dh, pix) = decode(data);
        assert_eq!((dw, dh), (w, h));
        for (k, &p) in px.iter().enumerate() {
            assert_eq!(&pix[k * 3..k * 3 + 3], &[(p >> 16) as u8, (p >> 8) as u8, p as u8], "pixel {}", k);
        }
    }

    #[test]
    fn png_of_a_screen_round_trips() {
        // runs longer than the 258 a match can take, rows equal to the row above, matches across the end of a row
        for (w, h) in [(1024u32, 600u32), (301, 97), (1, 1), (1, 300), (2, 3), (700, 1)] {
            let px = screen_like(w, h);
            assert_decodes_to(&png(w, h, &px), w, h, &px);
        }
        let flat = vec![rgb(1, 2, 3); 50 * 40];
        assert_decodes_to(&png(50, 40, &flat), 50, 40, &flat);
    }

    #[test]
    fn png_is_much_smaller_than_the_bitmap() {
        let (w, h) = (1024u32, 600u32);
        let raw = (w * 3 + 1) * h;
        let black = png(w, h, &vec![0u32; (w * h) as usize]);
        assert!(black.len() < raw as usize / 100, "black screen: {} bytes", black.len());
        let screen = png(w, h, &screen_like(w, h));
        assert!(screen.len() < raw as usize / 20, "screen-like picture: {} bytes of {}", screen.len(), raw);
    }

    /// Explicit cross-check with an independent decoder installed by the caller.
    #[test]
    #[ignore = "external tool: requires python3 with Pillow"]
    fn png_opens_in_pillow() {
        let (w, h) = (64u32, 40u32);
        let px: Vec<u32> = (0..w * h).map(|i| rgb((i % w) * 4, (i / w) * 6, 0x80)).collect();
        let dir = std::env::temp_dir().join(format!("oos-ui-png-{}", std::process::id()));
        let path = dir.join("t.png");
        write(&path, &png(w, h, &px)).unwrap();
        // a full screen with long runs and repeated rows, compared byte for byte with what Pillow decodes
        let (sw, sh) = (1024u32, 600u32);
        let screen = screen_like(sw, sh);
        let big = dir.join("big.png");
        write(&big, &png(sw, sh, &screen)).unwrap();
        let rgb_bytes: Vec<u8> = screen.iter().flat_map(|&p| [(p >> 16) as u8, (p >> 8) as u8, p as u8]).collect();
        write(&dir.join("big.rgb"), &rgb_bytes).unwrap();
        let script = "import sys\nfrom PIL import Image\nim = Image.open(sys.argv[1]); im.load()\nprint(im.size[0], im.size[1], im.mode, *im.getpixel((5, 7)))\nbig = Image.open(sys.argv[2]); big.load()\nprint(big.tobytes() == open(sys.argv[3], 'rb').read())";
        let out = std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(&path)
            .arg(&big)
            .arg(dir.join("big.rgb"))
            .output()
            .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(out.status.success(), "Pillow: {}", String::from_utf8_lossy(&out.stderr));
        let s = String::from_utf8_lossy(&out.stdout);
        assert_eq!(s.trim(), format!("64 40 RGB {} {} 128\nTrue", 5 * 4, 7 * 6));
    }

    /// A real screen (the demo, four channels and the menus of the UI) decodes back to its pixels.
    #[test]
    fn png_of_the_demo_screen_round_trips() {
        let mut ui = crate::ui::Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        for c in ["key ch2", "key ch3", "key ch4", "key measure"] {
            crate::cmd::run(&mut ui, c);
        }
        let mut cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        for _ in 0..3 {
            ui.step(0.04);
            ui.render(&mut cv);
        }
        assert_decodes_to(&png_of(&cv), cv.w as u32, cv.h as u32, &cv.px);
    }

    /// Time of one PNG of a real screen (the demo source, 4 channels on, as the web remote sends it) and of the
    /// screen-like test picture: `cargo test --release png_encode_time -- --ignored --nocapture`.
    #[test]
    #[ignore = "host timing: set OOS_PERF_TESTS=1"]
    fn png_encode_time() {
        assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"), "set OOS_PERF_TESTS=1");
        let mut ui = crate::ui::Ui::for_tests(crate::wave::Source::Demo { t: 0.0 });
        for c in ["key ch2", "key ch3", "key ch4"] {
            crate::cmd::run(&mut ui, c);
        }
        let mut cv = Canvas::new(crate::ui::PANEL_W, crate::ui::PANEL_H);
        for _ in 0..10 {
            ui.step(0.04);
            ui.render(&mut cv);
        }
        let synthetic = screen_like(cv.w as u32, cv.h as u32);
        for (name, px) in [("demo screen", &cv.px), ("screen-like", &synthetic)] {
            let n = 40;
            let mut size = 0;
            let t = std::time::Instant::now();
            for _ in 0..n {
                size = std::hint::black_box(png(cv.w as u32, cv.h as u32, px)).len();
            }
            let per = t.elapsed().as_secs_f64() / n as f64;
            println!("{}: {} x {}, {} bytes, {:.2} ms per PNG", name, cv.w, cv.h, size, per * 1e3);
            assert_decodes_to(&png(cv.w as u32, cv.h as u32, px), cv.w as u32, cv.h as u32, px);
        }
        // what the frame loop does per picture of the web remote now (http.rs `Shots`): one copy of the screen; and
        // what the encoder thread does before it encodes: compare it with the last copy
        let n = 200;
        let mut copy: Vec<u32> = Vec::with_capacity(cv.px.len());
        let t = std::time::Instant::now();
        for _ in 0..n {
            copy.clear();
            copy.extend_from_slice(std::hint::black_box(&cv.px));
        }
        let per_copy = t.elapsed().as_secs_f64() / n as f64;
        let t = std::time::Instant::now();
        let mut same = false;
        for _ in 0..n {
            same = crate::http::same_pixels(std::hint::black_box(&copy), std::hint::black_box(&cv.px));
        }
        let per_cmp = t.elapsed().as_secs_f64() / n as f64;
        assert!(same);
        println!(
            "frame loop: copy of the screen {:.3} ms; encoder thread: comparison {:.3} ms",
            per_copy * 1e3,
            per_cmp * 1e3
        );
    }

    #[test]
    fn next_free_name() {
        let dir = std::env::temp_dir().join(format!("oos-ui-names-{}", std::process::id()));
        assert_eq!(file_name(&next_name(&dir, "shot", "png")), "shot-001.png");
        write(&dir.join("shot-001.png"), b"x").unwrap();
        write(&dir.join("shot-002.png"), b"x").unwrap();
        assert_eq!(file_name(&next_name(&dir, "shot", "png")), "shot-003.png");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod header_import_tests {
    use super::*;
    #[test]
    fn other_waveform_headers_load_and_unknown_bytes_are_refused() {
        let samples = [0.0_f32, 1.0, -0.5, 0.25];
        let isf = isf_i16(0.0, 1e-9, 1e-3, &[0, 1000, -500, 250]);
        let got = parse_wave(&isf, "a.isf").unwrap();
        assert_close(&got[0], 0.0, 1e-9, &samples);

        let trc = wavedesc_i16(0.0, 1e-9, 1e-3, 0.0, &[0, 1000, -500, 250]);
        assert_close(&parse_wave(&trc, "a.trc").unwrap()[0], 0.0, 1e-9, &samples);

        let ag = ag_f32(-1e-6, 2e-9, &samples, "lane");
        let got = parse_wave(&ag, "a.bin").unwrap();
        assert_eq!(got[0].label, "lane");
        assert_close(&got[0], -1e-6, 2e-9, &samples);

        let tr0 = tr0_f32(0.0, 1e-9, &samples);
        assert_close(&parse_wave(&tr0, "a.tr0").unwrap()[0], 0.0, 1e-9, &samples);
        let mut ascii = String::from("00020000000000009601*x ");
        for name in ["TIME", "v(1)"] {
            ascii.push_str(&format!("{name:<16}"));
        }
        ascii.push_str("$&%#");
        for (k, x) in samples.iter().enumerate() {
            ascii.push_str(&format!("{:11.4e}{:11.4e}", k as f64 * 1e-9, x));
        }
        ascii.push_str(&format!("{:11.4e}", 1.0e30));
        assert_close(&parse_wave(ascii.as_bytes(), "c.tr0").unwrap()[0], 0.0, 1e-9, &samples);
        let raw = raw_f64(0.0, 1e-9, &samples);
        assert_close(&parse_wave(&raw, "b.tr0").unwrap()[0], 0.0, 1e-9, &samples);

        let wfmx = wfmx_f32(-1e-6, 1e-9, &samples);
        assert_close(&parse_wave(&wfmx, "a.wfmx").unwrap()[0], -1e-6, 1e-9, &samples);

        assert!(parse_wave(b"\x00\x01\x02\x03\xff", "no.bin").is_err());
        assert!(parse_wave(b"not a waveform\n", "no.csv").unwrap_err().contains("no waveform"));
        assert!(parse_wave(b"AGxx,1,2\n3,4,5\n", "no.bin").is_err());
    }

    #[test]
    fn header_import_isf_block_digits_and_point_counts_are_checked() {
        for byte in 0..=u8::MAX {
            let mut b = b":WFMPRE:BYT_NR 1;NR_PT 2;XINCR 1e-9;:CURVE #".to_vec();
            b.push(byte);
            b.extend_from_slice(b"000000000");
            assert!(parse_wave(&b, "bad.isf").is_err(), "block byte {byte}");
        }
        for field in ["NR_PT -1", "NR_PT 2.5", "NR_PT 18446744073709551615", "BYT_NR 1.5", "YMULT wrong"] {
            let b = format!(":WFMPRE:BYT_NR 1;XINCR 1e-9;{field};:CURVE #12\0\x01");
            assert!(parse_wave(b.as_bytes(), "bad.isf").is_err(), "{field}");
        }
        assert!(parse_wave(b":WFMPRE:BYT_NR 1;NR_PT 3;XINCR 1;:CURVE #12\0\x01", "short.isf")
            .unwrap_err()
            .contains("truncated"));
    }

    #[test]
    fn header_import_isf_envelope_preserves_pairs_and_requires_complete_data() {
        let header = b":WFMPRE:BYT_NR 1;NR_PT 2;PT_FMT ENV;XINCR 1;:CURVE #14";
        let mut b = header.to_vec();
        b.extend([-2i8 as u8, 2, -1i8 as u8, 1]);
        let traces = parse_wave(&b, "envelope.isf").unwrap();
        assert_eq!(traces.len(), 2);
        assert_eq!(traces[0].v, [-2.0, -1.0]);
        assert_eq!(traces[1].v, [2.0, 1.0]);
        let short = b":WFMPRE:BYT_NR 1;NR_PT 2;PT_FMT ENV;XINCR 1;:CURVE #13\0\x01\x02";
        assert!(parse_wave(short, "short.isf").unwrap_err().contains("truncated"));
    }

    #[test]
    fn header_import_fixed_width_unicode_is_safe_and_invalid_data_is_refused() {
        let text = format!("0002{}x$&%#", "é".repeat(16));
        let (nvars, names) = post_vars(&text).unwrap();
        assert_eq!((nvars, names.len()), (2, 2));
        let text = format!("00020000000000009601*x $&%#{}", "é".repeat(16));
        assert!(parse_wave(text.as_bytes(), "bad.tr0").is_err());
    }

    #[test]
    fn header_import_transient_requires_every_time_sample_to_be_uniform() {
        for time in [3.0, 1.0, f64::NAN, f64::INFINITY] {
            let mut b = raw_f64(0.0, 1.0, &[10.0, 20.0, 30.0, 40.0]);
            let start = b.len() - 4 * 16;
            b[start + 2 * 16..start + 2 * 16 + 8].copy_from_slice(&time.to_le_bytes());
            assert!(parse_wave(&b, "adaptive.tr0").unwrap_err().contains("nonuniform"));
        }
        let values = [10.0, 20.0, 30.0, 40.0];
        let b = raw_f64(-2e-6, 2e-9, &values);
        assert_close(&parse_wave(&b, "regular.tr0").unwrap()[0], -2e-6, 2e-9, &values);
    }

    #[test]
    fn header_import_wfmx_preserves_whitespace_bytes_at_binary_boundaries() {
        for first in [0x3f800020, 0x3f80000a, 0x3f80000d] {
            for last in [0x0a800000, 0x0d800000] {
                let values = [f32::from_bits(first), f32::from_bits(last)];
                let b = wfmx_f32(0.0, 1.0, &values);
                let trace = parse_wave(&b, "bytes.wfmx").unwrap().remove(0);
                assert_eq!(trace.v.iter().map(|v| v.to_bits()).collect::<Vec<_>>(), [first, last]);
                let mut crlf = b.clone();
                let separator = crlf.len() - 8 - 1;
                crlf.insert(separator, b'\r');
                assert_eq!(parse_wave(&crlf, "crlf.wfmx").unwrap()[0].v, values);
            }
        }
    }

    #[test]
    fn header_import_wfmx_embedded_payload_uses_exact_length_not_binary_tag_bytes() {
        let values = [f32::from_bits(0x3f800020), 2.0];
        let mut b = b"<?xml?><DataFile><FileType>WaveformInternal</FileType><Points>2</Points><XOrg>0</XOrg><XInc>1</XInc><Data>".to_vec();
        for value in values {
            b.extend(value.to_le_bytes());
        }
        b.extend_from_slice(b"</Data></DataFile>");
        assert_eq!(parse_wave(&b, "embedded.wfmx").unwrap()[0].v, values);
        let mut b = b"<Data>".to_vec();
        let payload = b"</Data>123456789";
        assert_eq!(payload.len(), 16);
        b.extend_from_slice(payload);
        b.extend_from_slice(b"</Data>");
        assert_eq!(wfmx_payload(&b, 4).unwrap(), payload);
        assert!(wfmx_payload(&b, usize::MAX).is_err());
    }

    #[test]
    fn header_import_wfmx_separator_cannot_replace_a_missing_payload_byte() {
        let mut b = wfmx_f32(0.0, 1.0, &[1.0, 2.0]);
        b.pop();
        assert!(parse_wave(&b, "short.wfmx").unwrap_err().contains("truncated"));
    }

    #[test]
    fn header_import_wfmx_checks_the_whole_explicit_time_axis() {
        let mut payload = Vec::new();
        for value in [0.0f64, 1.0, 3.0, 10.0, 20.0, 30.0] {
            payload.extend(value.to_le_bytes());
        }
        assert!(wfmx_volts(&payload, 3, 0.0, 1.0).is_err());
        payload[16..24].copy_from_slice(&2.0f64.to_le_bytes());
        assert_eq!(wfmx_volts(&payload, 3, 0.0, 1.0).unwrap(), [10.0, 20.0, 30.0]);
    }

    #[test]
    fn header_import_wfmx_f32_axis_accepts_quantization_without_guessing_other_layouts() {
        let n = 100_000;
        let t0 = -37e-6;
        let dt = 1e-9;
        let xs: Vec<f32> = (0..n).map(|i| (t0 + i as f64 * dt) as f32).collect();
        let ys: Vec<f32> = (0..n).map(|i| ((i % 37) as f32 - 18.0) / 19.0).collect();
        for interleaved in [false, true] {
            let mut payload = Vec::with_capacity(n * 8);
            if interleaved {
                for i in 0..n {
                    payload.extend(xs[i].to_le_bytes());
                    payload.extend(ys[i].to_le_bytes());
                }
            } else {
                for x in &xs {
                    payload.extend(x.to_le_bytes());
                }
                for y in &ys {
                    payload.extend(y.to_le_bytes());
                }
            }
            assert_eq!(wfmx_volts(&payload, n, t0, dt).unwrap(), ys);
            let at = if interleaved { (n - 1) * 8 } else { (n - 1) * 4 };
            payload[at..at + 4].copy_from_slice(&(xs[n - 1] + 10.0 * dt as f32).to_le_bytes());
            assert!(wfmx_volts(&payload, n, t0, dt).unwrap_err().contains("ambiguous"));
        }
        let implicit_f64: Vec<u8> = [1.0f64, 2.0, 3.0].into_iter().flat_map(f64::to_le_bytes).collect();
        assert!(wfmx_volts(&implicit_f64, 3, 0.0, 1.0).is_err());
    }

    #[test]
    fn header_import_long_tr0_f32_times_keep_source_precision_and_reject_adaptive_axis() {
        let values: Vec<f32> = (0..100_000).map(|i| (i % 7) as f32).collect();
        let b = tr0_f32(-37e-6, 1e-9, &values);
        let got = parse_wave(&b, "long.tr0").unwrap();
        assert_eq!(got[0].v, values);
        assert!((got[0].dt - 1e-9).abs() < 1e-16);
        let mut cols = Vec::new();
        for i in 0..1000 {
            cols.extend([(i as f64 * 1e-9) as f32 as f64, i as f64]);
        }
        cols[998 * 2] += 5e-9;
        assert!(traces_from_columns(&cols, 2, &[], true).unwrap_err().contains("nonuniform"));
    }

    #[test]
    fn header_import_known_formats_tolerate_all_truncation_boundaries_without_panicking() {
        let values = [0.0, 1.0, -0.5, 0.25];
        let fixtures = [
            isf_i16(0.0, 1e-9, 1e-3, &[0, 1000, -500, 250]),
            wavedesc_i16(0.0, 1e-9, 1e-3, 0.0, &[0, 1000, -500, 250]),
            ag_f32(0.0, 1e-9, &values, "lane"),
            tr0_f32(0.0, 1e-9, &values),
            raw_f64(0.0, 1e-9, &values),
            wfmx_f32(0.0, 1e-9, &values),
        ];
        for b in fixtures {
            for end in 0..b.len() {
                let _ = parse_wave(&b[..end], "header.bin");
            }
            assert_eq!(parse_wave(&b, "header.bin").unwrap()[0].v.len(), 4);
        }
    }

    fn assert_close(t: &WaveTrace, t0: f64, dt: f64, v: &[f32]) {
        let tol = dt.abs() * 1e-5 + 1e-15;
        assert!((t.t0 - t0).abs() <= tol, "t0 {} {}", t.t0, t0);
        assert!((t.dt - dt).abs() <= tol, "dt {} {}", t.dt, dt);
        assert_eq!(t.v.len(), v.len());
        for (a, b) in t.v.iter().zip(v) {
            assert!((a - b).abs() < 1e-5, "{a} {b}");
        }
    }

    fn isf_i16(t0: f64, dt: f64, ymult: f64, codes: &[i16]) -> Vec<u8> {
        let mut curve = Vec::new();
        for c in codes {
            curve.extend_from_slice(&c.to_le_bytes());
        }
        let len = curve.len().to_string();
        let mut b = format!(":WFMPRE:BYT_NR 2;ENCDG BIN;BN_FMT RI;BYT_OR LSB;NR_PT {};PT_FMT Y;XINCR {dt:.9e};XZERO {t0:.9e};PT_OFF 0;YMULT {ymult:.9e};YOFF 0;YZERO 0;WFID \"lane\";:CURVE #{}{len}", codes.len(), len.len()).into_bytes();
        b.extend(curve);
        b
    }

    fn wavedesc_i16(t0: f64, dt: f64, gain: f32, off: f32, codes: &[i16]) -> Vec<u8> {
        let desc = 346usize;
        let mut b = vec![0u8; desc + codes.len() * 2];
        b[..8].copy_from_slice(b"WAVEDESC");
        b[32] = 1;
        b[34] = 1;
        b[36..40].copy_from_slice(&(desc as u32).to_le_bytes());
        b[60..64].copy_from_slice(&((codes.len() * 2) as u32).to_le_bytes());
        b[116..120].copy_from_slice(&(codes.len() as u32).to_le_bytes());
        b[96..98].copy_from_slice(b"C1");
        b[156..160].copy_from_slice(&gain.to_le_bytes());
        b[160..164].copy_from_slice(&off.to_le_bytes());
        b[176..180].copy_from_slice(&(dt as f32).to_le_bytes());
        b[180..188].copy_from_slice(&t0.to_le_bytes());
        for (k, c) in codes.iter().enumerate() {
            b[desc + k * 2..desc + k * 2 + 2].copy_from_slice(&c.to_le_bytes());
        }
        b
    }

    fn ag_f32(t0: f64, dt: f64, v: &[f32], label: &str) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"AG10");
        let size_at = b.len();
        b.extend_from_slice(&0u32.to_le_bytes());
        b.extend_from_slice(&1u32.to_le_bytes());
        let mut h = vec![0u8; 140];
        h[..4].copy_from_slice(&140u32.to_le_bytes());
        h[4..8].copy_from_slice(&1u32.to_le_bytes());
        h[8..12].copy_from_slice(&1u32.to_le_bytes());
        h[12..16].copy_from_slice(&(v.len() as u32).to_le_bytes());
        h[32..40].copy_from_slice(&dt.to_le_bytes());
        h[40..48].copy_from_slice(&t0.to_le_bytes());
        h[112..112 + label.len()].copy_from_slice(label.as_bytes());
        b.extend(h);
        b.extend_from_slice(&12u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(&((v.len() * 4) as u32).to_le_bytes());
        for x in v {
            b.extend_from_slice(&x.to_le_bytes());
        }
        let n = b.len() as u32;
        b[size_at..size_at + 4].copy_from_slice(&n.to_le_bytes());
        b
    }

    fn tr0_block(payload: &[u8]) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend_from_slice(b"\x04\x00\x00\x00\x00\x00\x00\x00\x04\x00\x00\x00");
        b.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        b.extend_from_slice(payload);
        b.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        b
    }

    fn tr0_f32(t0: f64, dt: f64, v: &[f32]) -> Vec<u8> {
        let mut header = String::from("00020000000000009601*x ");
        for name in ["TIME", "v(1)"] {
            header.push_str(&format!("{name:<16}"));
        }
        header.push_str("$&%#");
        let mut data = Vec::new();
        for (k, x) in v.iter().enumerate() {
            let t = (t0 + k as f64 * dt) as f32;
            data.extend_from_slice(&t.to_le_bytes());
            data.extend_from_slice(&x.to_le_bytes());
        }
        data.extend_from_slice(&1.0e30_f32.to_le_bytes());
        let mut b = tr0_block(header.as_bytes());
        b.extend(tr0_block(&data));
        b
    }

    fn raw_f64(t0: f64, dt: f64, v: &[f32]) -> Vec<u8> {
        let mut b = format!("Title: t\nDate: today\nPlotname: Transient Analysis\nFlags: real\nNo. Variables: 2\nNo. Points: {}\nVariables:\n\t0\ttime\ttime\n\t1\tv(1)\tvoltage\nBinary:\n", v.len()).into_bytes();
        for (k, x) in v.iter().enumerate() {
            b.extend_from_slice(&(t0 + k as f64 * dt).to_le_bytes());
            b.extend_from_slice(&(*x as f64).to_le_bytes());
        }
        b
    }

    fn wfmx_f32(t0: f64, dt: f64, v: &[f32]) -> Vec<u8> {
        let mut b = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<DcaFile><Header><FileType>WaveformInternal</FileType></Header><Points>{}</Points><XOrg>{t0}</XOrg><XInc>{dt}</XInc><Data Lengths=\"{}, 1\"></Data></DcaFile>\n", v.len(), v.len()).into_bytes();
        for x in v {
            b.extend_from_slice(&x.to_le_bytes());
        }
        b
    }

    // Break it catches (rdtsc in the forum): a setup naming a protocol this build does not have, or a key of a newer
    // build, failing to load as a whole
    #[test]
    fn a_setup_loads_what_it_can() {
        let mut m = Model::default();
        m.dec_on = true;
        m.tscale = 5;
        // unknown keys were always passed over; a protocol this build lacks and a value out of range were not
        let text = setup_text(&m).replace("dec_proto = ", "dec_proto = NoSuchDecoder # was ") + "avg_hw = maybe\n";
        assert!(parse_setup(&text).is_err(), "strict parse names the first bad line");
        let (back, bad) = parse_setup_lenient(&text);
        assert_eq!(back.tscale, 5, "the rest of the setup stands");
        assert!(!back.dec_on, "the decoder without its protocol is off");
        assert!(bad.len() == 2 && bad[0].contains("dec_proto") && bad[1].contains("avg_hw"), "{:?}", bad);
        assert_eq!(parse_setup_lenient(&setup_text(&m)).1, Vec::<String>::new());
    }
}
