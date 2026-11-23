// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

// Timing guard (tools/perf-guard.sh): a worker frame drawn from the read-out words against decode + draw, lines,
// no per-frame measurements, also with AC trigger coupling (its level from the words). The A53 gains more than the
// host (the f32 copy is 2x the read-out per channel).
#[test]
#[ignore = "host timing: set OOS_PERF_TESTS=1"]
fn perf_guard_codes_per_frame() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"), "set OOS_PERF_TESTS=1");
    let worst = [
        (1u32, 100_000u32, 1e-5, TrigCoup::Dc),
        (3, 70_312, 1e-5, TrigCoup::Dc),
        (5, 50_000, 1e-5, TrigCoup::Dc),
        (1, 40_000, 2e-6, TrigCoup::Dc),
        (1, 100_000, 1e-5, TrigCoup::Ac),
    ]
    .into_iter()
    .map(|(mask, n, tdiv, coup)| measure_codes_per_frame(mask, n, tdiv, coup))
    .fold(0.0, f64::max);
    assert!(worst <= 0.9, "drawing from the codes must be cheaper: worst ratio {:.2}", worst);
}

fn measure_codes_per_frame(mask: u32, n: u32, tdiv: f64, coup: TrigCoup) -> f64 {
    let (w, h) = (970, 448);
    let sig = |k: usize, c: usize, i: usize| {
        0.9 * (((i as f64 + (k % 7) as f64 * 0.13) / 400.0) * (1.0 + c as f64 * 0.5) * std::f64::consts::TAU).sin()
    };
    let mut m = model(tdiv, false, false);
    m.trig_coup = coup;
    m.meas.clear();
    m.meas2.clear();
    for c in 0..NCH {
        m.ch[c].on = mask & (1 << c) != 0;
    }
    let frames = (2_000_000 / (n as usize * 2 * hz::ratio(mask) as usize)).clamp(4, 100);
    let (bytes, fb, s, p) = frames_of(mask, n, frames, &sig);
    let mut ph = Phosphor::new(w, h);
    let mut frame = Frame::default();
    let pipe = Pipe::start(1, 0, fresh());
    pipe.set_view(Some(&m), w, h, Interp::Exact);
    let cs = s.clone();
    let codes: CodesInto = Arc::new(move |_, bytes, f| crate::board::dho::hw::codes_into(bytes, &cs, &p, 0, f));
    let decode = dec(&s, &p);
    let mut buffer = Some(Box::new(Bytes(bytes.clone())));
    let mut draw_codes = || {
        let bytes = buffer.take().expect("the previous round returned its buffer");
        pipe.feed_codes(bytes, fb, frames, m.hw_stamp, decode.clone(), Some(codes.clone()));
        buffer = Some(recycle_worker_round(&pipe, frames));
    };
    draw_codes();
    // one round: every frame once (each round's time divided by the frames below)
    let (old, new) = crate::perfguard::guard_pair(
        0.9,
        crate::perfguard::ROUNDS,
        1,
        || {
            for chunk in bytes.chunks_exact(fb) {
                crate::board::dho::hw::decode_into(chunk, &s, &p, 0, &mut frame);
                std::hint::black_box(draw_frame(&mut ph, &m, &frame, Interp::Exact, None));
            }
        },
        draw_codes,
    );
    assert_eq!(pipe.counters.read().2, 0);
    let (old, new) = (old / frames as u32, new / frames as u32);
    let r = new.as_secs_f64() / old.as_secs_f64();
    println!(
        "codes mask {:#x} {} pt {:?}: decode + draw {:?}/frame, from the codes {:?}/frame (ratio {:.2})",
        mask, n, coup, old, new, r
    );
    r
}

fn recycle_worker_round(pipe: &Pipe, frames: usize) -> Box<Bytes> {
    let buffer = recycled(pipe);
    // The fixture has one batch in flight. Consume its handoff without timing the UI fold.
    let deposit = pipe.slot.take().expect("worker published the deposit before returning the buffer");
    assert_eq!(deposit.records, frames as u64);
    pipe.slot.recycle(deposit);
    buffer
}

/// Host cost per frame of a worker (decode into kept vectors, trigger refinement, deposit) against the frames
/// mode's per-frame work on the UI thread (`decode` + `Ui::fast_draw`), and the rate of 1, 2 and 4 workers (the
/// helper runs the same code per frame). Run by tools/perf-guard.sh.
#[test]
#[ignore = "host timing: set OOS_PERF_TESTS=1"]
fn perf_guard_deposit_worker_per_frame() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"), "set OOS_PERF_TESTS=1");
    let (w, h) = (970, 448);
    for (tdiv, memsel, meas) in [
        (1e-7, 1408u32, false),
        (1e-6, 12_512, false),
        (1e-6, 12_512, true),
        (1e-5, 125_000, false),
        (1e-5, 125_000, true),
    ] {
        let mut m = model(tdiv, false, false);
        // the measurements made on every frame cost on top (reported, not guarded): the worst case, all of them on
        m.meas = if meas {
            crate::framestats::CHEAP.iter().map(|&k| (k, crate::model::WaveId::Ch(0))).collect()
        } else {
            Vec::new()
        };
        let frames = (2_000_000 / (memsel as usize * 2)).clamp(8, 500);
        let (bytes, fb, s, p) = batch(frames, memsel, false);
        // the frames mode: decode + fast_draw on one thread (its measurements: a few records a second), against
        // one worker of the pipeline, alternating (perfguard.rs); 2 and 4 workers are reported only
        let mut ph = Phosphor::new(w, h);
        let one = Pipe::start(1, 0, fresh());
        one.set_view(Some(&m), w, h, Interp::Exact);
        // Copy once outside timing, then recycle through the real pool: 101 rounds still use one buffer.
        let mut buffer = Some(Box::new(Bytes(bytes.clone())));
        let (old, one_t) = crate::perfguard::guard_pair(
            if meas { f64::INFINITY } else { 1.5 },
            crate::perfguard::ROUNDS,
            1,
            || {
                for chunk in bytes.chunks_exact(fb) {
                    std::hint::black_box(draw_frame(
                        &mut ph,
                        &m,
                        &crate::board::dho::hw::decode(chunk, &s, &p, 0),
                        Interp::Exact,
                        None,
                    ));
                }
            },
            || {
                one.feed(buffer.take().expect("the previous round returned its buffer"), fb, frames, 0, dec(&s, &p));
                buffer = Some(recycle_worker_round(&one, frames));
            },
        );
        assert_eq!(one.counters.read().2, 0);
        let (old, mut per) = (old / frames as u32, vec![one_t / frames as u32]);
        for n in [2usize, 4] {
            let pipe = Pipe::start(n, 0, fresh());
            pipe.set_view(Some(&m), w, h, Interp::Exact);
            let t0 = std::time::Instant::now();
            for _ in 0..3 {
                pipe.feed(Box::new(Bytes(bytes.clone())), fb, frames, 0, dec(&s, &p));
                drop(recycle_worker_round(&pipe, frames));
            }
            per.push(t0.elapsed() / (3 * frames) as u32);
            assert_eq!(pipe.counters.read().2, 0);
        }
        let r = per[0].as_secs_f64() / old.as_secs_f64();
        println!("{} s/div {} pt, frame statistics {}: frames mode {:?}/frame; one core in the pipeline {:?}/frame (ratio {:.2}); 1/2/4 workers {:.0}/{:.0}/{:.0} frames/s", tdiv, memsel, meas, old, per[0], r, 1.0 / per[0].as_secs_f64(), 1.0 / per[1].as_secs_f64(), 1.0 / per[2].as_secs_f64());
        assert!(meas || r <= 1.5, "a frame in the pipeline must not cost more than in the frames mode: {:.2}", r);
    }
}
