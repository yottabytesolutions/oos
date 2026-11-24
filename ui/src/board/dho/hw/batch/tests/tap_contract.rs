// SPDX-License-Identifier: GPL-2.0-or-later
//! Host workload evidence, not a production admission budget or a minimum-cycle proof.

use super::*;
use crate::board::PictureEligibility;
use oos_hal::dho::seq;

fn backing(frame: &Frame) -> usize {
    std::mem::size_of::<Frame>() + frame.ch.iter().map(|ch| ch.capacity() * std::mem::size_of::<f32>()).sum::<usize>()
}

#[test]
fn tap_contract_actual_descriptor_backing_includes_raw_replay() {
    for compress in [false, true] {
        for tdiv in [1e-6, 10e-6] {
            let (_, _, mut b, p) = normal_lines_plan(tdiv, compress);
            b.header = false;
            let fb = seq::frame_bytes(&b, &p);
            let decoded = tap::storage(&b, fb).unwrap();
            let raw = seq::raw_settings(&b);
            let replay = tap::storage(&raw, seq::frame_bytes(&raw, &p)).unwrap();
            let mandatory = super::super::super::plan::raw_newest(&b, &p);
            let owned = if mandatory { backing(&replay).max(backing(&decoded)) } else { backing(&decoded) };
            assert_eq!(decoded.ch.each_ref().map(Vec::len), [0; 4]);
            assert!(decoded.ch.iter().all(|ch| ch.capacity() > 0));
            if mandatory {
                assert!(backing(&replay) > backing(&decoded));
            }
            println!(
                "TAP_GEOMETRY tdiv={tdiv} compress={compress} depth={} rate={} k={} frame_bytes={fb} decoded={} replay={} mandatory={mandatory} owned={owned}",
                p.memsel, p.rate, seq::compress_k(&b, &p), backing(&decoded), backing(&replay),
            );
        }
    }
}

#[test]
fn tap_contract_scripted_pulses_preserve_fifo_and_whole_pictures() {
    for compress in [false, true] {
        for (tdiv, records_per_second) in [(1e-6, 1770usize), (10e-6, 400)] {
            for records in [1, 2] {
                pulses(tdiv, compress, records, records_per_second.div_ceil(30 * records) + 3);
            }
        }
    }
}

fn pulses(tdiv: f32, compress: bool, records: usize, batches: usize) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let counters = crate::drawpipe::counters();
    let (m, sw, mut b, p) = normal_lines_plan(tdiv, compress);
    (b.header, b.frames) = (false, records as u32);
    let (input, _, _) = scripted::draw_route_batch(&m, &sw, 0);
    let fb = seq::frame_bytes(&b, &p);
    let bytes = input.into_bytes();
    assert!(bytes.bytes().len() >= fb);
    let pipe = Pipe::start(1, 0, counters);
    let id = Identity::default();
    let size = (970, 448);
    pipe.publish_view(&m, size.0, size.1, sw.interp, id);
    let interval = std::time::Duration::from_micros(500);
    pipe.fund_taps(
        crate::picture::deposit::admission::Share::checked(128 << 20, 0, 128 << 20, 256, std::sync::Arc::new(()))
            .unwrap(),
        interval,
    )
    .unwrap();
    assert!(tap::admit(&pipe, &b, &p, id));
    assert_eq!(pipe.tap_drain(), 70);
    let origin = Instant::now();
    let mut picture = Phosphor::new(size.0, size.1);
    let mut sequence = 0;
    for window in 0..3 {
        assert!(picture.maps().iter().all(|ch| ch.iter().all(|&px| px == 0)));
        for batch in 0..batches {
            let at = origin + std::time::Duration::from_nanos(window as u64 * 33_333_334) + interval * batch as u32;
            assert_eq!(pipe.tap_start(id, at), Some(std::time::Duration::ZERO));
            sequence += records as u64;
            submit_pulse(&pipe, (&b, &p), &bytes.bytes()[..fb], (id, sequence), records);
        }
        assert_eq!(pipe.tap_count(), batches);
        // Read every FIFO item before settlement. Pending records must not paint again.
        for batch in 0..batches {
            let tap = pipe.take_waveform(size).unwrap();
            assert_eq!(tap.seq, (window * batches + batch + 1) as u64 * records as u64);
            assert_eq!(tap.eligibility(), PictureEligibility::Pending);
        }
        assert!(pipe.take_waveform(size).is_none());
        let mut accepted = 0;
        // One published and one coalesced deposit: fixed bounded work, no timing polling.
        for _ in 0..2 {
            crate::drawpipe::host::barrier(&pipe);
            accepted += pipe.accept(&mut picture, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames;
        }
        assert_eq!(accepted, (batches * records) as u64);
        assert!(!picture.is_wide(), "normal Lines retain narrow counts");
        let before = picture.maps().clone();
        crate::drawpipe::host::barrier(&pipe);
        assert_eq!(pipe.accept(&mut picture, &mut false, &mut [0; 4], &mut (0.0, 0.0)).frames, 0);
        assert_eq!(picture.maps(), &before, "settlement must not draw the pending preview twice");
        picture.clear();
    }
    assert_eq!(counters.placed.load(std::sync::atomic::Ordering::Relaxed), sequence);
    assert_eq!(counters.read().2, 0);
    println!("TAP_PULSES tdiv={tdiv} compress={compress} records={records} windows=3 batches_per_window={batches} records_total={sequence} accept_calls_per_window=3");
}

pub(super) fn submit_pulse(
    pipe: &Pipe,
    (b, p): (&Settings, &hz::Params),
    bytes: &[u8],
    (id, sequence): (Identity, u64),
    n: usize,
) {
    let reservation = pipe.reserve_plan(id).unwrap();
    let raw = super::super::super::plan::raw_newest(b, p).then(|| {
        let raw = seq::raw_settings(b);
        let bytes = vec![0; seq::frame_bytes(&raw, p)];
        super::super::super::decode_probed(&bytes, &raw, p, sequence, HeaderProbe::Off)
    });
    let mut dma = Some(Bytes(bytes.repeat(n)));
    let result =
        Readout::new(pipe, (b, p), (bytes.len(), n, false), (sequence, HeaderProbe::Off, raw), (id, Instant::now()))
            .reserved(Some(reservation))
            .dispatch(&mut dma, None, |f| f, &|| false);
    assert!(result.is_empty());
    finish(pipe);
}
