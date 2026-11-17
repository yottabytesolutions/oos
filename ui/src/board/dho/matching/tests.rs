// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::dho::switches::HeaderProbe;

fn setup(mask: u32, header: bool, n: u32) -> (Settings, hz::Params) {
    let mut s = Settings { mask, header, compress: 1, ..Settings::default() };
    for (c, ch) in s.ch.iter_mut().enumerate() {
        ch.scale = 0.17 * (c + 1) as f64;
        ch.offset = 0.031 * c as f64;
    }
    let mut p = hz::prep(mask, 1_000_000_000, n as i128, 0);
    p.memsel = n;
    (s, p)
}

fn record(s: &Settings, p: &hz::Params, frame: usize) -> Vec<u8> {
    let map = seq::code_map(s.mask, s.ch.map(|c| (c.scale, c.offset)));
    let mut bytes = if s.header { vec![0xa5; 16] } else { Vec::new() };
    let pattern = [80, -40, 20, -60, -20, 60, -80, 40];
    for i in 0..p.memsel as usize {
        for lane in 0..map.ratio {
            let c = map.word.iter().position(|&w| w == Some(lane)).unwrap_or(lane);
            let offset = pattern[(i + frame * 3 + c) % 8] * (10 + frame as i32 % 5) / 10;
            let wave = (i as f64 * 0.019 + frame as f64).sin() * 3000.0;
            let word = (32768 + wave as i32 + offset) as u16;
            bytes.extend(word.to_le_bytes());
        }
    }
    bytes
}

/// Pin upstream's original de-interleave, match, f64 volts conversion as an independent oracle.
fn standard(bytes: &[u8], s: &Settings, p: &hz::Params, cores: &mut Cores) -> Frame {
    let mut words = seq::deinterleave(payload(bytes, s), s.mask);
    let ch = std::array::from_fn(|c| {
        cores.apply((s.mask, c, p.rate), &mut words[c]);
        seq::volts(&words[c], s.ch[c].scale, s.ch[c].offset)
    });
    Frame { ch, ..Frame::default() }
}

#[test]
fn batch_core_matching_equals_standard_in_record_order() {
    for mask in [1, 2, 3, 5, 7, 15] {
        for header in [false, true] {
            let (mut batch_cores, mut standard_cores, mut live_cores) =
                (Cores::default(), Cores::default(), Cores::default());
            for n in [4097, 255, 8192, 1003, 70_000, 4097] {
                let (s, p) = setup(mask, header, n);
                let records: Vec<_> = (0..8).map(|i| record(&s, &p, i)).collect();
                let bytes: Vec<_> = records.concat();
                let fb = records[0].len();
                let expected: Vec<_> = records.iter().map(|r| standard(r, &s, &p, &mut standard_cores)).collect();
                for (i, r) in records.iter().enumerate() {
                    let f = super::super::hw::decode_with(r, &s, &p, 0, HeaderProbe::Off, Some(&mut live_cores));
                    assert_eq!(f.ch, expected[i].ch, "Standard changed: {mask} {header} {n} {i}");
                }
                let batch = BatchMatch::learn(&bytes, fb, 0..records.len(), &s, &p, &mut batch_cores);
                assert_eq!(
                    batch
                        .newest(records.last().expect("the fixture has eight records"), &s, &p, 7, HeaderProbe::Off)
                        .ch,
                    expected[7].ch
                );
                let decode = batch.decoder(s.clone(), p, HeaderProbe::Off);
                // Workers may complete in any order; each frame keeps the correction learned for it.
                std::thread::scope(|scope| {
                    for (i, r) in records.iter().enumerate().rev() {
                        let (decode, expected) = (&decode, &expected);
                        scope.spawn(move || {
                            let mut f = Frame::default();
                            decode(i, r, &mut f);
                            assert_eq!(f.ch, expected[i].ch, "Batch: {mask} {header} {n} {i}");
                        });
                    }
                });
            }
        }
    }
}

#[test]
fn batch_core_matching_corrects_neighbours_and_freezes_snapshots() {
    let (s, p) = setup(1, true, 4096);
    let records: Vec<_> = (0..12).map(|i| record(&s, &p, i)).collect();
    let mut cores = Cores::default();
    let batch = BatchMatch::learn(&records.concat(), records[0].len(), 0..records.len(), &s, &p, &mut cores);
    // Further acquisition must not change corrections of an in-flight batch.
    for i in 20..40 {
        FrameMatch::learn(&record(&s, &p, i), &s, &p, &mut cores);
    }
    let decode = batch.decoder(s.clone(), p, HeaderProbe::Off);
    let mut oracle = Cores::default();
    for (i, r) in records.iter().enumerate() {
        let mut frame = Frame::default();
        decode(i, r, &mut frame);
        assert_eq!(frame.ch, standard(r, &s, &p, &mut oracle).ch);
        let raw = super::super::hw::decode_probed(r, &s, &p, 0, HeaderProbe::Off);
        assert_ne!(frame.ch[0], raw.ch[0], "uncorrected neighbour {i}");
    }
}

#[test]
fn batch_core_matching_excludes_pairs_and_peak() {
    let (s, p) = setup(1, false, 4096);
    for s in [Settings { compress: 8, ..s.clone() }, Settings { mode: Mode::Peak, ..s }] {
        assert!(!eligible(&s));
        let r = record(&s, &p, 0);
        let mut cores = Cores::default();
        let f = super::super::hw::decode_with(&r, &s, &p, 0, HeaderProbe::Off, Some(&mut cores));
        assert_eq!(f.ch, super::super::hw::decode_probed(&r, &s, &p, 0, HeaderProbe::Off).ch);
        assert!(cores.pattern((s.mask, 0, p.rate)).is_none());
    }
}

struct Bytes(Vec<u8>);
impl crate::drawpipe::RawBatch for Bytes {
    fn bytes(&self) -> &[u8] {
        &self.0
    }
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any + Send> {
        self
    }
}

#[test]
fn batch_core_matching_workers_draw_the_standard_picture() {
    use crate::{
        drawpipe,
        model::{tscales, Model},
        sparse::Interp,
        wave::Phosphor,
    };
    let (s, p) = setup(1, false, 40_000);
    let records: Vec<_> = (0..10).map(|i| record(&s, &p, i)).collect();
    let bytes = records.concat();
    let fb = records[0].len();
    // Above CODES_MIN_BYTES: this would silently draw uncorrected codes if preparation kept that shortcut.
    assert!(fb >= drawpipe::CODES_MIN_BYTES);
    let mut m = Model {
        tscale: tscales()
            .iter()
            .position(|&t| (t as f64 - 1e-6).abs() < 1e-9)
            .expect("the standard timebase table includes 1 us/div"),
        ..Model::default()
    };
    for (c, ch) in m.ch.iter_mut().enumerate() {
        ch.on = c == 0;
    }
    let mut oracle = Cores::default();
    let mut want = Phosphor::new(128, 64);
    let mut last = Frame::default();
    for (i, r) in records.iter().enumerate() {
        let f = super::super::hw::decode_with(r, &s, &p, 0, HeaderProbe::Off, Some(&mut oracle));
        if i < records.len() - 1 {
            drawpipe::draw_frame(&mut want, &m, &f, Interp::Exact, None);
        } else {
            last = f;
        }
    }
    for workers in [1, 2] {
        let pipe = drawpipe::Pipe::start(workers, 0, Box::leak(Box::default()));
        pipe.publish_view(&m, 128, 64, Interp::Exact, Default::default());
        let mut cores = Cores::default();
        let (matched, newest) =
            prepare(&bytes, (fb, records.len(), 0), &s, &p, (10, HeaderProbe::Off, None), Some(&mut cores));
        assert_eq!(newest.ch, last.ch);
        let delivered = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done = delivered.clone();
        feed(
            &pipe,
            (Box::new(Bytes(bytes.clone())), fb, 0..9),
            Default::default(),
            (s.clone(), p, HeaderProbe::Off),
            matched,
            Box::new(move || done.store(true, std::sync::atomic::Ordering::Release)),
        );
        finish(&pipe);
        let mut got = Phosphor::new(128, 64);
        let n = pipe.merge(&mut got, &mut false, &mut [0; 4], &mut (0.0, 0.0));
        assert_eq!(n.frames, 9);
        assert_eq!(got.map, want.map);
        assert!(delivered.load(std::sync::atomic::Ordering::Acquire));
    }
}

#[test]
fn batch_core_matching_keeps_rejected_frames_and_clamps_codes() {
    let (s, p) = setup(3, false, 4097);
    let mut records: Vec<_> = (0..32).map(|i| record(&s, &p, i)).collect();
    for r in &mut records[2..20] {
        for (i, b) in r.as_chunks_mut::<2>().0.iter_mut().enumerate() {
            let word = (32768.0 + 15_000.0 * (i as f64 * std::f64::consts::TAU / 16.0).sin()) as u16;
            b.copy_from_slice(&word.to_le_bytes());
        }
    }
    // Both clamp ends and an incomplete final interleave group.
    records[1][..8].copy_from_slice(&[0, 0, 255, 255, 0, 0, 255, 255]);
    for r in &mut records {
        r.truncate(r.len() - 3);
    }
    let mut cores = Cores::default();
    let batch = BatchMatch::learn(&records.concat(), records[0].len(), 0..records.len(), &s, &p, &mut cores);
    let decode = batch.decoder(s.clone(), p, HeaderProbe::Off);
    let mut oracle = Cores::default();
    for (i, r) in records.iter().enumerate() {
        let mut f = Frame::default();
        decode(i, r, &mut f);
        assert_eq!(f.ch, standard(r, &s, &p, &mut oracle).ch, "frame {i}");
    }
}

fn finish(pipe: &crate::drawpipe::Pipe) {
    let mut helper = crate::drawpipe::Helper::default();
    let deadline = crate::ui::lifecycle::test_wait::Wait::new("batch workers return the completed buffer");
    while pipe.take_buffer().is_none() {
        deadline.check();
        if !pipe.help(&mut helper) {
            deadline.poll();
        }
        pipe.help_done(&mut helper);
    }
}

/// Full 4 MB batches, including the small frames that pay estimator and snapshot costs most often.
#[test]
#[ignore = "timing guard, run by tools/perf-guard.sh"]
fn perf_guard_batch_core_learning() {
    use crate::system::perfguard;
    for (mask, points, frames) in [(1, 2000, 1000), (3, 1000, 1000), (15, 1000, 500)] {
        let (s, p) = setup(mask, false, points);
        let bytes = (0..frames).flat_map(|i| record(&s, &p, i)).collect::<Vec<_>>();
        assert_eq!(bytes.len(), 4_000_000);
        let fb = bytes.len() / frames;
        let (mut cores, mut oracle) = (Cores::default(), Cores::default());
        // Warm both estimator windows and keys; BatchMatch still allocates its per-batch snapshots.
        BatchMatch::learn(&bytes, fb, 0..frames, &s, &p, &mut cores);
        for chunk in bytes.chunks_exact(fb) {
            let mut words = seq::deinterleave(chunk, mask);
            for (c, words) in words.iter_mut().enumerate() {
                oracle.apply((mask, c, p.rate), words);
            }
        }
        let pair = perfguard::measure(
            perfguard::ROUNDS,
            3,
            || {
                for chunk in bytes.chunks_exact(fb) {
                    let mut words = seq::deinterleave(chunk, mask);
                    for (c, words) in words.iter_mut().enumerate() {
                        oracle.apply((mask, c, p.rate), words);
                    }
                    std::hint::black_box(words);
                }
            },
            || {
                std::hint::black_box(BatchMatch::learn(&bytes, fb, 0..frames, &s, &p, &mut cores));
            },
        );
        eprintln!(
            "BatchMatch::learn 4 MB mask={mask}: best {:?}, p99 CPU {:?}; estimated A72 p99 {:?} (4x, unverified)",
            pair.best.1,
            pair.p99.1,
            pair.p99.1 * 4
        );
        // Host p99 1.415 ms x the perf plan's 4x A72 factor x 1.5 headroom, rounded up to 9 ms.
        // This is a provisional projection, not measured unit timing.
        assert!(pair.p99.1 * 4 <= std::time::Duration::from_millis(9), "projected A72 learning exceeds 9 ms/4 MB");
        pair.report("perf_guard_batch_core_learning");
        assert!(pair.best.1 <= pair.best.0, "batch learning exceeds Standard learning plus correction");
        pair.check_tail(1.5);
    }
}
