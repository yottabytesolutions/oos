// SPDX-License-Identifier: GPL-2.0-or-later

use super::{DATA_LIMIT, META_LIMIT, VERSION};
use crate::board::dho::cores::{Cores, Pattern, P};
use crate::board::dho::switches::FrameZero;
use crate::board::Frame;
use crate::model::NCH;
use oos_core::la::LaRec;
use oos_hal::dho::{
    acq::Mode,
    hz,
    seq::{self, Playback},
    tpu,
};
use std::{collections::BTreeMap, fs, io::Read, ops::Range, path::Path, str::FromStr, sync::Arc};

#[cfg(test)]
mod capture_tests;
mod trigger;
mod view;
use view::view;
pub(crate) use view::PictureView;

type Fields<'a> = BTreeMap<&'a str, &'a str>;
type ReadGeometry = (usize, usize, FrameZero);

pub(crate) struct Fixture {
    name: String,
    settings: seq::Settings,
    params: hz::Params,
    frame_bytes: usize,
    bytes: Arc<[u8]>,
    matching: bool,
    patterns: [Option<Pattern>; NCH],
    newest: Option<Arc<Frame>>,
    dispatch: Dispatch,
}

pub(in crate::board::dho::hw) struct Dispatch {
    frame0: FrameZero,
    avail: usize,
    la: Option<LaRec>,
    submitted: Option<Range<usize>>,
    route: Option<String>,
    view: Option<PictureView>,
}

impl Fixture {
    pub(in crate::board::dho::hw) fn load(path: &Path) -> Result<Self, String> {
        let meta = String::from_utf8(read(path, "fixture.txt", META_LIMIT)?).map_err(|e| e.to_string())?;
        let (fields, route) = parse(&meta)?;
        let (settings, params) = geometry(&fields)?;
        let (frames, frame_bytes, stride): (usize, usize, usize) =
            (value(&fields, "frames")?, value(&fields, "frame_bytes")?, value(&fields, "frame_stride")?);
        let bytes = read(path, "dma.bin", DATA_LIMIT)?;
        if frame_bytes != seq::frame_bytes(&settings, &params)
            || stride != frame_bytes
            || stride != seq::frame_stride(frame_bytes)
            || frame_bytes == 0
            || !(1..=1000).contains(&frames)
            || frames.checked_mul(stride) != Some(bytes.len())
        {
            return Err("invalid DMA size, frame count or stride".into());
        }
        let patterns = [
            pattern(&fields, "core0")?,
            pattern(&fields, "core1")?,
            pattern(&fields, "core2")?,
            pattern(&fields, "core3")?,
        ];
        let name = path.file_name().and_then(|s| s.to_str()).ok_or("fixture directory needs a UTF-8 name")?.into();
        Ok(Self {
            name,
            settings,
            params,
            frame_bytes,
            bytes: bytes.into(),
            patterns,
            newest: read_newest(path, &fields)?.map(Arc::new),
            matching: value(&fields, "matching")?,
            dispatch: dispatch(path, &fields, route, frames)?,
        })
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn view(&self) -> Option<&PictureView> {
        self.dispatch.view.as_ref()
    }

    pub(in crate::board::dho::hw) fn settings(&self) -> (&seq::Settings, &hz::Params) {
        (&self.settings, &self.params)
    }

    pub(in crate::board::dho::hw) fn source(&self) -> (Arc<[u8]>, ReadGeometry, Option<Frame>) {
        (
            self.bytes.clone(),
            (self.frame_bytes, self.dispatch.avail, self.dispatch.frame0),
            self.newest.as_deref().cloned(),
        )
    }

    pub(in crate::board::dho::hw) fn la(&self) -> Option<&LaRec> {
        self.dispatch.la.as_ref()
    }

    pub(crate) fn expected_route(&self) -> Option<(&str, Range<usize>)> {
        Some((self.dispatch.route.as_deref()?, self.dispatch.submitted.clone()?))
    }

    pub(in crate::board::dho::hw) fn cores(&self) -> Cores {
        let mut cores = Cores::default();
        for (c, pattern) in self.patterns.into_iter().enumerate() {
            if let Some(pattern) = pattern {
                cores.restore_fixture_pattern((self.settings.mask, c, self.params.rate), pattern);
            }
        }
        cores
    }

    pub(in crate::board::dho::hw) fn matching(&self) -> bool {
        self.matching
    }

    pub(in crate::board::dho::hw) fn patterns(&self) -> [Option<Pattern>; NCH] {
        self.patterns
    }

    pub(in crate::board::dho::hw) fn records(&self) -> impl Iterator<Item = &[u8]> {
        self.bytes.chunks_exact(self.frame_bytes)
    }
}

fn read(path: &Path, name: &str, limit: u64) -> Result<Vec<u8>, String> {
    let mut contents = Vec::new();
    fs::File::open(path.join(name))
        .and_then(|f| f.take(limit + 1).read_to_end(&mut contents))
        .map_err(|e| format!("{name}: {e}"))?;
    if contents.len() as u64 > limit {
        return Err(format!("{name} is too large"));
    }
    Ok(contents)
}

fn parse(meta: &str) -> Result<(Fields<'_>, Option<&str>), String> {
    let mut lines = meta.lines();
    if lines.next() != Some(VERSION) {
        return Err("unsupported fixture version".into());
    }
    let mut fields = Fields::new();
    let mut route = None;
    for line in lines {
        if line.starts_with("route.v1 ") {
            if route.replace(line).is_some() {
                return Err("duplicate route.v1".into());
            }
        } else {
            let (key, val) = line.split_once('=').ok_or("missing field separator")?;
            if fields.insert(key.trim(), val.trim()).is_some() {
                return Err(format!("duplicate field {key}"));
            }
        }
    }
    field(&fields, "build")?;
    Ok((fields, route))
}

fn geometry(fields: &Fields<'_>) -> Result<(seq::Settings, hz::Params), String> {
    let mask = value(fields, "mask")?;
    let p = horizontal(fields, mask)?;
    let [has_window, points, start] = words(fields, "window")?;
    let mut s = seq::Settings {
        mask,
        header: value(fields, "header")?,
        compress: value(fields, "compression")?,
        playback: if value(fields, "play4")? { Playback::Raw } else { Playback::Last },
        window: cast::<bool>("window", has_window)?.then_some((
            cast("window points", points)?,
            i64::from(cast::<u32>("window start", start)?) - i64::from(p.pre),
        )),
        tdiv: finite(fields, "tdiv")?,
        delay: finite(fields, "delay")?,
        level: finite(fields, "level")?,
        trig_src: value(fields, "source")?,
        slope: match field(fields, "slope")? {
            "Rising" => tpu::Slope::Rising,
            "Falling" => tpu::Slope::Falling,
            "Either" => tpu::Slope::Either,
            _ => return Err("invalid slope".into()),
        },
        trig_couple: match field(fields, "coupling")? {
            "Dc" => tpu::Coupling::Dc,
            "Ac" => tpu::Coupling::Ac,
            "HfReject" => tpu::Coupling::HfReject,
            "LfReject" => tpu::Coupling::LfReject,
            _ => return Err("invalid coupling".into()),
        },
        mode: match field(fields, "mode")? {
            "normal" => Mode::Normal,
            "peak" => Mode::Peak,
            "average" => Mode::Average { count: value(fields, "average")? },
            _ => return Err("invalid ADC mode".into()),
        },
        trigger: trigger::parse(field(fields, "trigger")?)?,
        holdoff: finite(fields, "holdoff")?,
        trigger_options: seq::TriggerOptions::new(false, value(fields, "noise_reject")?),
        ..seq::Settings::default()
    };
    restore_channels(fields, &mut s.ch)?;
    restore_capture(fields, &mut s, p)?;
    if seq::compress_k(&s, &p) != value(fields, "effective_compression")?
        || seq::window(&s, &p) != (cast("window points", points)?, cast("window start", start)?)
    {
        return Err("inconsistent compression or read window".into());
    }
    Ok((s, p))
}

fn restore_channels(fields: &Fields<'_>, channels: &mut [seq::Chan; NCH]) -> Result<(), String> {
    for (c, channel) in channels.iter_mut().enumerate() {
        let [scale, offset, invert, couple, bw] = words(fields, &format!("channel{c}"))?;
        channel.scale = finite_str("scale", scale)?;
        channel.offset = finite_str("offset", offset)?;
        channel.invert = cast("invert", invert)?;
        channel.couple = match couple {
            "Dc" => seq::Couple::Dc,
            "Ac" => seq::Couple::Ac,
            "Gnd" => seq::Couple::Gnd,
            _ => return Err("invalid channel coupling".into()),
        };
        channel.bw = cast("bandwidth", bw)?;
    }
    Ok(())
}

fn restore_capture(fields: &Fields<'_>, s: &mut seq::Settings, p: hz::Params) -> Result<(), String> {
    let Some(capture) = fields.get("capture") else { return Ok(()) };
    let (mode, factor) = capture.split_once(':').ok_or("invalid capture reduction")?;
    let peak = match (mode, s.mode) {
        ("decimate", Mode::Normal) => false,
        ("peak", Mode::Peak) => true,
        _ => return Err("inconsistent capture and ADC mode".into()),
    };
    s.depth = value(fields, "capture_base_depth")?;
    if !(0..=125_000_000).contains(&s.depth) || !(0.0..=0.2).contains(&s.tdiv) {
        return Err("invalid capture base depth or time base".into());
    }
    s.mode = Mode::Normal;
    let planned = s.reduce_capture(peak, cast("capture factor", factor)?, value(fields, "capture_inferred")?);
    if planned != Some(p) {
        return Err("inconsistent capture reduction geometry".into());
    }
    Ok(())
}

fn horizontal(fields: &Fields<'_>, mask: u32) -> Result<hz::Params, String> {
    let p = hz::Params {
        ratio: value(fields, "ratio")?,
        rate: value(fields, "rate")?,
        memsel: value(fields, "stored_points")?,
        dot_fs: value(fields, "dot_fs")?,
        seg: value(fields, "segment")?,
        max_frames: value(fields, "max_frames")?,
        pre: value(fields, "pre")?,
        post: value(fields, "post")?,
    };
    if !(1..=15).contains(&mask) || p.ratio != hz::ratio(mask) || p.rate == 0 || !(1..=125_000_000).contains(&p.memsel)
    {
        return Err("invalid channel mask, sampling rate or stored points".into());
    }
    Ok(p)
}

fn dispatch(path: &Path, fields: &Fields<'_>, route: Option<&str>, frames: usize) -> Result<Dispatch, String> {
    let avail = value(fields, "avail")?;
    if avail > frames {
        return Err("dispatched more records than completed".into());
    }
    let (submitted, view) = match field(fields, "dispatch")? {
        "batch" => {
            let [start, end] = words(fields, "submitted")?.map(|v| cast("submitted range", v));
            let range = start?..end?;
            if range.start > range.end || range.end > avail || route.is_none() {
                return Err("invalid worker range or route".into());
            }
            (Some(range), view(fields)?)
        }
        "records" if route.is_none() => (None, None),
        _ => return Err("invalid dispatch or route.v1".into()),
    };
    let drop_first = value::<bool>(fields, "frame0_drop")?;
    let check = fields.contains_key("frame0_check") && value::<bool>(fields, "frame0_check")?;
    if drop_first && check {
        return Err("conflicting frame-zero policies".into());
    }
    Ok(Dispatch {
        avail,
        frame0: if check { FrameZero::Steady } else { drop_first.into() },
        la: read_la(path, fields)?,
        submitted,
        route: route.map(str::to_owned),
        view,
    })
}

fn pattern(fields: &Fields<'_>, key: &str) -> Result<Option<Pattern>, String> {
    if !fields.contains_key(key) {
        return Ok(None);
    }
    let [samples, misfit, offsets @ ..] = words::<{ P + 2 }>(fields, key)?;
    let mut l = [0.0; P];
    for (out, input) in l.iter_mut().zip(offsets) {
        *out = finite_str(key, input)?;
    }
    Ok(Some(Pattern::new(l, finite_str(key, samples)?, cast(key, misfit)?)))
}

fn read_newest(path: &Path, fields: &Fields<'_>) -> Result<Option<Frame>, String> {
    if !fields.contains_key("newest") {
        return Ok(None);
    }
    let [dt, pre, rate, mem, envelope, exact, counts @ ..] = words::<10>(fields, "newest")?;
    let bytes = read(path, "newest.bin", DATA_LIMIT)?;
    let mut iter = bytes.as_chunks::<4>().0.iter();
    let mut ch = std::array::from_fn(|_| Vec::new());
    for (lane, n) in ch.iter_mut().zip(counts) {
        let n = cast::<usize>("newest lane count", n)?;
        if iter.len() < n {
            return Err("truncated newest waveform".into());
        }
        lane.extend(iter.by_ref().take(n).map(|w| f32::from_le_bytes(*w)));
    }
    if iter.len() != 0 || bytes.len() % 4 != 0 {
        return Err("extra bytes in newest waveform".into());
    }
    let [roll, scan, partial, fixed_ti] = words(fields, "newest_flags")?;
    Ok(Some(Frame {
        roll: cast("newest roll", roll)?,
        scan: cast("newest scan", scan)?,
        partial: cast("newest partial", partial)?,
        fixed_ti: (fixed_ti != "none").then(|| finite_str("newest trigger", fixed_ti)).transpose()?,
        dt: finite_str("newest dt", dt)?,
        pre: cast("newest pre", pre)?,
        acq: (finite_str("newest rate", rate)?, cast("newest stored points", mem)?),
        envelope: cast("newest envelope", envelope)?,
        trigger_anchor: crate::board::TriggerAnchor::new(cast("newest pre exact", exact)?),
        ch,
        ..Frame::default()
    }))
}

fn read_la(path: &Path, fields: &Fields<'_>) -> Result<Option<LaRec>, String> {
    if !fields.contains_key("la") {
        return Ok(None);
    }
    let [count, t0, dt] = words(fields, "la")?;
    let bytes = read(path, "la.bin", DATA_LIMIT)?;
    if cast::<usize>("LA sample count", count)?.checked_mul(2) != Some(bytes.len()) {
        return Err("invalid LA data size".into());
    }
    Ok(Some(LaRec {
        words: Arc::new(bytes.as_chunks::<2>().0.iter().copied().map(u16::from_le_bytes).collect()),
        t0: finite_str("LA origin", t0)?,
        dt: finite_str("LA interval", dt)?,
    }))
}

fn words<'a, const N: usize>(fields: &Fields<'a>, key: &str) -> Result<[&'a str; N], String> {
    field(fields, key)?.split_ascii_whitespace().collect::<Vec<_>>().try_into().map_err(|_| format!("invalid {key}"))
}

fn field<'a>(fields: &Fields<'a>, key: &str) -> Result<&'a str, String> {
    fields.get(key).copied().ok_or_else(|| format!("missing {key}"))
}

fn cast<T: FromStr>(key: &str, val: &str) -> Result<T, String> {
    val.parse().map_err(|_| format!("invalid {key}"))
}

fn value<T: FromStr>(fields: &Fields<'_>, key: &str) -> Result<T, String> {
    cast(key, field(fields, key)?)
}

fn finite(fields: &Fields<'_>, key: &str) -> Result<f64, String> {
    finite_str(key, field(fields, key)?)
}

fn finite_str(key: &str, val: &str) -> Result<f64, String> {
    let v: f64 = cast(key, val)?;
    v.is_finite().then_some(v).ok_or_else(|| format!("nonfinite {key}"))
}
