// SPDX-License-Identifier: GPL-2.0-or-later

//! Synthetic read-outs with the DHO layout and captured decode metadata. No device or calibration files.

mod records;

use super::hw;
use crate::board::scripted::{Batch, BatchKind, BatchSource, ScriptedBoard};
use crate::board::Frame;
use crate::drawpipe::{CodesInto, DecodeInto, RawBatch};
use crate::model::Model;
use oos_hal::dho::{hz, seq};
use std::sync::Arc;

#[derive(Default)]
pub struct ScriptBytes(Vec<u8>);

impl RawBatch for ScriptBytes {
    fn bytes(&self) -> &[u8] {
        &self.0
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send> {
        self
    }
}

pub struct ScriptedBatchSource {
    kind: BatchKind,
    seed: u64,
    seq: u64,
}

pub struct ScriptBatch {
    bytes: ScriptBytes,
    fb: usize,
    frames: usize,
    settings: seq::Settings,
    params: hz::Params,
    first: u64,
    codes: bool,
    kind: BatchKind,
    tdiv: f64,
}

impl ScriptedBatchSource {
    pub fn new(kind: BatchKind, seed: u64) -> Self {
        Self { kind, seed, seq: 0 }
    }
}

impl ScriptedBoard for super::Dho {
    type Source = ScriptedBatchSource;

    fn scripted_source(kind: BatchKind, seed: u64) -> Self::Source {
        ScriptedBatchSource::new(kind, seed)
    }
}

impl BatchSource for ScriptedBatchSource {
    type Bytes = ScriptBytes;
    type Batch = ScriptBatch;

    fn next_batch(
        &mut self,
        m: &Model,
        switches: &crate::sysconf::Switches,
        frames: usize,
        reuse: Option<ScriptBytes>,
    ) -> Result<ScriptBatch, String> {
        if !(1..=32).contains(&frames) {
            return Err("scripted batch must contain 1..32 frames".into());
        }
        let mut s = hw::settings(m, false, switches);
        s.header = false;
        s.window = None;
        s.compress = 1;
        match self.kind {
            BatchKind::LivePairs => s.depth = 100_000,
            BatchKind::Roll | BatchKind::Scan => s.depth = 12_500,
            BatchKind::DeepPairs => s.depth = 10_000_000,
            // Slow Auto normally streams. This source supplies bounded records on the existing queue instead.
            _ if s.depth == 0 => s.depth = 100_000,
            _ => {}
        }
        let p = hz::prep(s.mask, seq::tdiv_fs(s.tdiv), s.depth, seq::tdiv_fs(s.delay));
        s.compress = match self.kind {
            BatchKind::LivePairs => 16,
            BatchKind::DeepPairs => p.memsel.div_ceil(12_500),
            _ => 1,
        };
        let fb = seq::frame_bytes(&s, &p);
        if fb == 0 || fb > (32 << 20) / frames {
            return Err(format!("scripted read-out exceeds 32 MiB: {} x {}", frames, fb));
        }
        let mut bytes = reuse.unwrap_or_default();
        bytes.0.clear();
        fill(&mut bytes.0, frames, &s, &p, &mut self.seed);
        let first = self.seq + 1;
        self.seq += frames as u64;
        Ok(ScriptBatch {
            bytes,
            fb,
            frames,
            settings: s,
            params: p,
            first,
            codes: self.kind == BatchKind::Codes,
            kind: self.kind,
            tdiv: m.time_per_div() as f64,
        })
    }
}

fn fill(out: &mut Vec<u8>, frames: usize, s: &seq::Settings, p: &hz::Params, seed: &mut u64) {
    let map = seq::code_map(s.mask, s.ch.map(|c| (c.scale, c.offset)));
    let points = seq::frame_bytes(s, p) / (2 * map.ratio);
    let k = seq::compress_k(s, p) as usize;
    out.reserve(frames * points * map.ratio * 2);
    for frame in 0..frames {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        let jitter = (*seed % 7) as i64 - 3;
        for point in 0..points {
            for lane in 0..map.ratio {
                let channel = map.word.iter().position(|w| *w == Some(lane)).unwrap_or(0);
                let sample = if k > 1 { point / 2 * 2 * k } else { point };
                let phase = (sample as i64 - p.pre as i64 + jitter).rem_euclid(512);
                let triangle = if phase < 256 { phase - 128 } else { 384 - phase };
                let pulse = if frame % 13 == 0 && phase < 4 { 6000 } else { 0 };
                let envelope = if k > 1 {
                    if point % 2 == 0 {
                        -900
                    } else {
                        900
                    }
                } else {
                    0
                };
                let word = (32768 + triangle * (40 + channel as i64 * 3) + pulse + envelope) as u16;
                out.extend_from_slice(&word.to_le_bytes());
            }
        }
    }
}

impl Batch for ScriptBatch {
    type Bytes = ScriptBytes;
    fn frame_bytes(&self) -> usize {
        self.fb
    }
    fn newest(&self, stamp: u64) -> Frame {
        self.frame(self.frames - 1, stamp)
    }
    fn frame(&self, index: usize, stamp: u64) -> Frame {
        let mut f = Frame::default();
        hw::decode_into_probed(
            &self.bytes.0[index * self.fb..(index + 1) * self.fb],
            &self.settings,
            &self.params,
            self.first + index as u64,
            &mut f,
            false,
        );
        f.stamp = stamp;
        records::characterise(&mut f, self.kind, self.tdiv);
        f
    }
    fn decode(&self) -> DecodeInto {
        let (s, p) = (self.settings.clone(), self.params);
        Arc::new(move |bytes, f| hw::decode_into_probed(bytes, &s, &p, 0, f, false))
    }
    fn codes(&self) -> Option<CodesInto> {
        let (s, p) = (self.settings.clone(), self.params);
        self.codes
            .then(|| Arc::new(move |bytes: &[u8], f: &mut Frame| hw::codes_into(bytes, &s, &p, 0, f)) as CodesInto)
    }
    fn into_bytes(self) -> ScriptBytes {
        self.bytes
    }
}

/// Unit-shaped deep pairs using the production read plan, rather than the small M1 golden fixture.
#[cfg(test)]
pub(crate) fn deep_intake_frame(m: &Model, sw: &crate::sysconf::Switches) -> Frame {
    let mut s = hw::settings(m, false, sw);
    s.header = false;
    let p = hz::prep(s.mask, seq::tdiv_fs(s.tdiv), s.depth, seq::tdiv_fs(s.delay));
    let b = hw::batch_plan(&s, &p, sw.board.zone_batch);
    let mut bytes = Vec::new();
    fill(&mut bytes, 1, &b, &p, &mut 123);
    let f = hw::decode(&bytes, &b, &p, 1);
    assert_eq!(f.acq.1, 10_000_000);
    assert_eq!((b.compress, b.frames), (16, 1));
    assert_eq!(f.ch.each_ref().map(Vec::len), [625_000; 4]);
    assert!(f.envelope);
    println!(
        "deep fixture: depth={} k={} bytes={} rate={} dt={} pre={}",
        p.memsel,
        b.compress,
        bytes.len(),
        p.rate,
        f.dt,
        f.pre
    );
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_decode_metadata_and_seed_are_repeatable() {
        let m = Model::default();
        for kind in [BatchKind::Raw, BatchKind::Codes, BatchKind::LivePairs, BatchKind::DeepPairs] {
            let mut a = ScriptedBatchSource::new(kind, 123);
            let mut b = ScriptedBatchSource::new(kind, 123);
            let batch = a.next_batch(&m, &Default::default(), 3, None).unwrap();
            let same = b.next_batch(&m, &Default::default(), 3, None).unwrap();
            assert_eq!(batch.bytes.0, same.bytes.0);
            assert_eq!(batch.bytes.0.len(), batch.fb * 3);
            let f = batch.newest(7);
            assert_eq!((f.seq, f.stamp), (3, 7));
            assert_eq!(f.envelope, matches!(kind, BatchKind::LivePairs | BatchKind::DeepPairs));
            assert_eq!(f.acq, (batch.params.rate as f64, batch.params.memsel as usize));
            assert_eq!(f.ch[0], same.newest(7).ch[0]);
            if f.envelope {
                assert!(f.ch.iter().all(|ch| ch.as_chunks::<2>().0.iter().all(|pair| pair[0] <= pair[1])));
            }
            if kind == BatchKind::DeepPairs {
                assert_eq!(f.acq.1, 10_000_000);
            }
        }
    }

    #[test]
    fn codes_use_direct_drawing_and_four_channels_and_dots_fall_back() {
        use crate::drawpipe::{draw_frame_codes, CODES_MAX_STRIDE, CODES_MIN_BYTES};
        use crate::model::DotsMode;
        use crate::wave::Phosphor;
        let mut m = Model::default();
        let mut src = ScriptedBatchSource::new(BatchKind::Codes, 123);
        for (channels, dots, direct) in
            [(1, DotsMode::Off, true), (2, DotsMode::Auto, true), (4, DotsMode::Off, false), (1, DotsMode::On, false)]
        {
            for c in 0..4 {
                m.ch[c].on = c < channels;
            }
            m.dots = dots;
            let batch = src.next_batch(&m, &Default::default(), 3, None).unwrap();
            let mut f = Frame::default();
            let layout = batch.codes().unwrap()(&batch.bytes.0[..batch.fb], &mut f).unwrap();
            assert!(batch.fb >= CODES_MIN_BYTES);
            assert_eq!(layout.stride <= CODES_MAX_STRIDE, channels < 4);
            let result = (layout.stride <= CODES_MAX_STRIDE)
                .then(|| {
                    draw_frame_codes(&mut Phosphor::new(970, 448), &m, &f, &batch.bytes.0[..batch.fb], &layout, None)
                })
                .flatten();
            assert_eq!(result.is_some(), direct);
        }
    }

    #[test]
    fn slow_auto_depth_supplies_bounded_records_with_captured_metadata() {
        let m = Model { tscale: crate::model::tscales().iter().position(|&t| t == 0.2).unwrap(), ..Default::default() };
        let mut src = ScriptedBatchSource::new(BatchKind::Raw, 123);
        let batch = src.next_batch(&m, &Default::default(), 27, None).unwrap();
        assert!(batch.bytes.0.len() <= 32 << 20);
        assert_eq!(batch.newest(0).acq.1, 100_000);
    }

    #[test]
    fn batch_bounds_and_buffer_reuse_are_explicit() {
        let mut src = ScriptedBatchSource::new(BatchKind::Raw, 123);
        for n in [0, 33, usize::MAX] {
            assert!(src.next_batch(&Model::default(), &Default::default(), n, None).is_err());
        }
        let batch = src.next_batch(&Model::default(), &Default::default(), 3, None).unwrap();
        let ptr = batch.bytes.0.as_ptr();
        let next = src.next_batch(&Model::default(), &Default::default(), 3, Some(batch.into_bytes())).unwrap();
        assert_eq!(ptr, next.bytes.0.as_ptr());
        assert_eq!(next.newest(0).seq, 6);
        assert!(BatchKind::parse("invalid").is_err());
    }
}
