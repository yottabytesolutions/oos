// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::{
    fastdraw::{CorrectedCodes, Samples},
    picture::deposit::kernel::{self, Projection, RawTrace, SamplesRef, TrigOutcome, Vertical},
    wave::Rng,
};
use oos_core::model::HDIV;
use oos_hal::dho::samples::{code_map, volt_code, CodeMap};

mod bench;

const CAL: [(f64, f64); 4] = [(1.0, 0.125), (-0.5, -0.25), (1.25, 0.375), (0.75, 0.5125)];

struct Record<const N: usize> {
    words: Vec<[u16; N]>,
    bytes: Vec<u8>,
    layout: CodeMap,
    lanes: LaneMap<N>,
    offsets: Option<[[i32; N]; PHASES]>,
    correction: Correction<N>,
}

impl<const N: usize> Record<N> {
    fn new(mask: u32, words: Vec<[u16; N]>, offsets: Option<[[i32; N]; PHASES]>) -> Self {
        let layout = code_map(mask, CAL);
        assert_eq!(layout.ratio, N);
        let lanes = LaneMap::new(layout.word, mask).unwrap();
        let bytes = words.as_flattened().iter().flat_map(|word| word.to_le_bytes()).collect();
        Self { words, bytes, layout, lanes, offsets, correction: Correction::new(offsets.unwrap_or([[0; N]; PHASES])) }
    }

    fn codes(&self, channel: usize) -> Option<Codes<'_>> {
        let word = self.layout.word[channel]?;
        Some(Codes {
            bytes: &self.bytes,
            words: Some(self.words.as_flattened()),
            stride: N,
            word,
            n: self.words.len(),
            affine: self.layout.affine[channel],
            corrected: self.offsets.map(|offsets| CorrectedCodes {
                offsets: offsets.map(|row| row[word]),
                scale: CAL[channel].0,
                offset: CAL[channel].1,
            }),
            volt: volt_code,
        })
    }

    fn placed_spans(&self, proj: &Projection, ti: f64, previous: Option<RawTrace>) -> RawTrace {
        let samples = std::array::from_fn(|c| self.codes(c).map_or(SamplesRef::Volts(&[]), SamplesRef::Codes));
        let layout = crate::drawpipe::CodeLayout::uncorrected(N, self.layout.word, self.layout.affine, volt_code);
        kernel::place_at(
            samples,
            proj,
            (crate::drawpipe::route::Route::Dense, Layout::new(&layout)),
            (ti, TrigOutcome::Hardware),
            previous,
        )
        .trace
    }

    fn projection(&self, width: usize, height: usize, samples_per_col: f64, x0: f64) -> Projection {
        Projection {
            cols: width,
            rows: height,
            time_per_div: 1.0,
            x_per_sample: samples_per_col.recip(),
            delay: (width as f64 / 2.0 - x0) * HDIV as f64 / width.max(1) as f64,
            ch: std::array::from_fn(|c| {
                self.layout.word[c].map(|_| Vertical { volts_per_div: 0.75 + c as f32 * 0.5, offset: c as f32 * 0.3 })
            }),
            dots: false,
            format: Default::default(),
        }
    }
}

fn words<const N: usize>(rng: &mut Rng, len: usize) -> Vec<[u16; N]> {
    (0..len).map(|_| std::array::from_fn(|_| rng.next() as u16)).collect()
}

fn random_offsets<const N: usize>(rng: &mut Rng, wide: bool) -> [[i32; N]; PHASES] {
    let limit = if wide { 65_535 } else { 250 };
    std::array::from_fn(|_| std::array::from_fn(|_| (rng.next() % (2 * limit + 1)) as i32 - limit as i32))
}

fn compare<const N: usize>(record: &Record<N>, proj: &Projection, ti: f64, fused: bool) {
    let width = proj.cols;
    let channel = (0..4).find(|&c| proj.ch[c].is_some()).unwrap();
    let mut edges = ColumnEdges::default();
    let mut out = Envelopes::with_capacity(0);
    edges.update(width, record.words.len(), &kernel::raw_map(proj, channel, ti).unwrap());
    envelope(&record.words, &record.correction, &edges, &record.lanes, &mut out);
    let old = (fused && !record.words.is_empty()).then(|| record.placed_spans(proj, ti, None));
    for lane in 0..N {
        let Some(channel) = record.lanes.channel(lane) else { continue };
        let codes = record.codes(channel).unwrap();
        let map = kernel::raw_map(proj, channel, ti).unwrap();
        let mut rows = Spans::default();
        let mut expected = Spans::default();
        spans_into(&out, lane, &codes, &map, proj.rows, &mut RowCache::default(), &mut rows);
        fastdraw::spans_into(width, proj.rows, &codes, &map, &mut expected);
        let context = (N, channel, codes.n, map, proj.rows);
        assert_eq!((rows.c0, &rows.rows), (expected.c0, &expected.rows), "{context:?}");
        let decoded: Vec<_> = (0..codes.n).map(|i| codes.at(i)).collect();
        fastdraw::spans_into(width, proj.rows, decoded.as_slice(), &map, &mut expected);
        assert_eq!((rows.c0, &rows.rows), (expected.c0, &expected.rows), "{context:?}");
        if let Some(placed) = &old {
            let old = placed.spans[channel].as_ref().unwrap();
            assert_eq!((rows.c0, &rows.rows), (old.c0, &old.rows), "{context:?}");
        }
        check_codes(&out, &edges, lane, codes);
    }
}

fn check_codes<const N: usize>(out: &Envelopes<N>, edges: &ColumnEdges, lane: usize, codes: Codes<'_>) {
    for (column, &[a, b]) in out.columns.iter().zip(edges.ranges().first.array_windows()) {
        let values = (joined(a, b, true)..b).map(|i| codes.at_code(i));
        assert_eq!(column.low[lane], values.clone().min().unwrap_or(u16::MAX), "lane {lane}, {a}..{b}");
        assert_eq!(column.high[lane], values.max().unwrap_or(u16::MIN), "lane {lane}, {a}..{b}");
    }
}

fn randomized<const N: usize>(mask: u32, fused: bool) {
    let mut rng = Rng::new(329 + u64::from(mask));
    for width in 1..=40 {
        for trial in 0..12 {
            let len = if trial < 8 { trial } else { 64 + rng.next() as usize % 1200 };
            let samples_per_col = width as f64 + if trial % 2 == 0 { 0.0 } else { rng.uniform() };
            let words = words(&mut rng, len);
            let offsets = (trial % 3 != 0).then(|| random_offsets(&mut rng, trial % 3 == 1));
            let record = Record::<N>::new(mask, words, offsets);
            let x0 = rng.uniform() * 128.0 - 64.0;
            let ti = (rng.uniform() - 0.3) * (len + 1) as f64;
            let cols = 1 + rng.next() as usize % 256;
            let rows = 1 + rng.next() as usize % 720;
            compare(&record, &record.projection(cols, rows, samples_per_col, x0), ti, fused);
        }
    }
}

#[test]
fn randomized_envelopes_match_code_and_decoded_spans() {
    for mask in 1u32..16 {
        match mask.count_ones() {
            1 => randomized::<1>(mask, false),
            2 => randomized::<2>(mask, false),
            _ => randomized::<4>(mask, false),
        }
    }
}

#[test]
fn four_lanes_match_fused_spans() {
    randomized::<4>(0b1111, true);
}

#[test]
fn sparse_columns_and_both_clamps_keep_predecessors() {
    let words = vec![[20, 65500]; 21];
    let offsets = std::array::from_fn(|phase| [32768 + phase as i32, -32769 - phase as i32]);
    let record = Record::new(0b1010, words, Some(offsets));
    assert_eq!([record.lanes.channel(0), record.lanes.channel(1)], [Some(1), Some(3)]);
    for samples_per_col in [0.4, 1.0, 3.1, 6.25, 8.0, 10.0, 40.0] {
        for phase in 0..PHASES {
            compare(&record, &record.projection(45, 480, samples_per_col, 0.31), phase as f64, false);
        }
    }
}

#[test]
fn warm_buffers_survive_changing_the_visible_columns_without_allocating() {
    let width = 1024;
    let mut rng = Rng::new(256);
    let record = Record::new(0b1101, words(&mut rng, 3520), Some(random_offsets::<4>(&mut rng, true)));
    let proj = record.projection(width, 480, 3.1, 512.0);
    let codes = record.codes(0).unwrap();
    let mut edges = ColumnEdges::with_capacity(width);
    let mut out = Envelopes::with_capacity(width);
    let mut spans = Spans { c0: 0, rows: Vec::with_capacity(width) };
    let mut cache = RowCache::default();
    let map = kernel::raw_map(&proj, 0, 0.0).unwrap();
    cache.get(&codes, &map, proj.rows).unwrap();
    edges.update(width, record.words.len(), &map);
    let (allocations, ()) = crate::system::test_allocator::count_own(|| {
        for ti in [2000.0, 0.0, 99999.0, 1500.0, 3000.0] {
            let map = kernel::raw_map(&proj, 0, ti).unwrap();
            edges.update(width, record.words.len(), &map);
            envelope(&record.words, &record.correction, &edges, &record.lanes, &mut out);
            spans_into(&out, 0, &codes, &map, proj.rows, &mut cache, &mut spans);
            std::hint::black_box(&spans);
        }
    });
    assert_eq!(allocations, 0);
    spans_into(&out, 1, &codes, &kernel::raw_map(&proj, 0, 0.0).unwrap(), proj.rows, &mut cache, &mut spans);
    assert_eq!((spans.c0, spans.rows.len()), (0, 0));
}

#[test]
fn interleaved_code_pairs_match_decoded_pairs_in_every_lane() {
    let mut rng = Rng::new(151);
    let data = words::<4>(&mut rng, 813);
    let offsets = random_offsets(&mut rng, true);
    let record = Record::new(0b1111, data, Some(offsets));
    let mut scratch = Scratch::default();
    let inputs = std::array::from_fn(|c| SamplesRef::Codes(record.codes(c).unwrap()));
    let (mut actual, mut expected) = (std::array::from_fn(|_| None), Spans::default());
    let mut projection = record.projection(117, 480, 7.125, 59.0);
    for (ti, dots) in [(0.0, false), (240.21, false), (410.0, true), (812.0, false)] {
        projection.dots = dots;
        scratch.pairs((&inputs, Some(Layout::Four(record.lanes))), &projection, ti, &mut actual);
        for (c, span) in actual.iter().enumerate() {
            let codes = record.codes(c).unwrap();
            let decoded: Vec<_> = (0..codes.n).map(|i| codes.at(i)).collect();
            let map = kernel::raw_map(&projection, c, ti).unwrap();
            crate::compress::reference_spans(projection.cols, projection.rows, &decoded, &map, dots, &mut expected);
            let span = span.as_ref().unwrap();
            assert_eq!((span.c0, &span.rows), (expected.c0, &expected.rows), "channel={c} ti={ti} dots={dots}");
        }
    }
}

#[test]
fn scalar_reduction_clamps_the_range_and_keeps_record_relative_phase() {
    let words = [[100]; 3];
    let correction = Correction::new(std::array::from_fn(|phase| [phase as i32]));
    for (start, end, expected) in
        [(1, 8, Extrema { low: [98], high: [99] }), (2, 1, Extrema::EMPTY), (6, 8, Extrema::EMPTY)]
    {
        assert_eq!(scalar::reduce(&words, &correction, start..end), expected);
    }
    let correction = Correction::new([[i32::MIN]; PHASES]);
    assert_eq!(correction.row([0], 0), [u16::MAX]);
}

#[cfg(target_arch = "aarch64")]
#[test]
fn masked_neon_columns_match_scalar_at_both_record_ends() {
    fn check<const N: usize>() {
        let words: [[u16; N]; 19] =
            std::array::from_fn(|i| std::array::from_fn(|lane| (i * 3413 + lane * 2377) as u16));
        for magnitude in [0, 320, 65_536, i32::MAX] {
            let correction = Correction::new(std::array::from_fn(|phase| {
                std::array::from_fn(|lane| if (phase + lane) % 2 == 0 { magnitude } else { -magnitude })
            }));
            let mut actual = Vec::new();
            for len in [0, 1, 3, 8, 19] {
                let words = &words[..len];
                for start in 0..=len + 1 {
                    for end in start..=len + 2 {
                        for join in [false, true] {
                            let expected = scalar::reduce(words, &correction, joined(start, end, join)..end);
                            actual.clear();
                            neon::reduce(words, &correction, &[start, end], join, &mut actual);
                            assert_eq!(actual, [expected], "lanes={N} len={len} range={start}..{end} join={join}");
                        }
                    }
                }
            }
        }
    }
    check::<1>();
    check::<2>();
    check::<4>();
}

#[cfg(target_arch = "aarch64")]
#[test]
fn neon_matches_sample_correction_at_every_phase_and_clamp_boundary() {
    use crate::board::correct_adc_code;

    fn check<const N: usize>() {
        let mut actual = Vec::new();
        for pattern in [
            [0; PHASES],
            [1, -1, 17, -17, 32768, -32767, 803, -509],
            [32769, -32768, 65535, -65535, 65536, -65536, 97, -113],
        ] {
            let correction = Correction::new(std::array::from_fn(|phase| {
                std::array::from_fn(|lane| pattern[(phase + lane) % PHASES])
            }));
            for case in 0..5 {
                let words: [[u16; N]; 35] = std::array::from_fn(|i| {
                    correction.offsets[i % PHASES].map(|offset| {
                        let boundary = if offset < 0 { 65535 + offset } else { offset }.clamp(0, 65535) as u16;
                        [0, boundary.saturating_sub(1), boundary, boundary.saturating_add(1), u16::MAX][case]
                    })
                });
                for (i, &row) in words.iter().enumerate() {
                    let offsets = correction.offsets[i % PHASES];
                    let expected = std::array::from_fn(|lane| correct_adc_code(row[lane], offsets[lane]));
                    assert_eq!(correction.row(row, i), expected);
                }
                for len in [1, 3, 7, 9, words.len()] {
                    for start in 0..len {
                        for end in start + 1..=len {
                            for join in [false, true] {
                                let first = start.saturating_sub(usize::from(join));
                                let expected = scalar::reduce(&words[..len], &correction, first..end);
                                actual.clear();
                                neon::reduce(&words[..len], &correction, &[start, end], join, &mut actual);
                                assert_eq!(
                                    actual,
                                    [expected],
                                    "lanes={N} len={len} range={start}..{end} join={join} case={case}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    check::<1>();
    check::<2>();
    check::<4>();
}
