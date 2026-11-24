// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

// Break it catches: live compression where the model needs every sample, a factor that leaves fewer than two
// pairs per column, compression above 1 M points (the deep-memory read), a batch that does not grow with it.
#[test]
// Break it catches, end to end on the host (unit 2026-11-14): 10 M at 1 ms/div, CH2 alone, Vavg on CH2. The pair
// frames of the batch hold the measurement, the raw screen window the plan reads gives it; without a planned window
// the UI held for a raw record that never came and showed `----`.
fn deep_record_gives_vavg_from_the_raw_window() {
    use crate::wave::Source;
    let shared: crate::board::HwShared = Default::default();
    let mut ui = crate::ui::Ui::for_tests(Source::Hw { shared: shared.clone(), seen: 0 });
    let m = &mut ui.m;
    m.run = RunState::Run;
    m.tscale = tscales().iter().position(|&t| (t as f64 - 1e-3).abs() < 1e-9).unwrap();
    m.mdepth = mdepths().iter().position(|d| d.1 == 10_000_000).unwrap();
    for c in 0..NCH {
        m.ch[c].on = c == 1;
    }
    m.meas = vec![(Meas::Vavg, WaveId::Ch(1))];
    let s = settings(&ui.m, false, &Default::default());
    let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, oos_hal::dho::seq::tdiv_fs(s.delay));
    let b = batch_plan(&s, &p, 1);
    let d = deep_raw_settings(&b, &p).expect("a raw window for CH2 at 10 M");
    // what the acquisition thread publishes with the batch
    shared.lock().unwrap().deep_ok = true;
    let pairs = decode(&vec![0x5au8; oos_hal::dho::seq::frame_bytes(&b, &p)], &b, &p, 1);
    let raw = decode(&vec![0x5au8; oos_hal::dho::seq::frame_bytes(&d, &p)], &d, &p, 2);
    assert!(crate::deepmeas::deep_pairs(&pairs) && crate::deepmeas::deep_raw_frame(&raw));
    let level = raw.ch[1][raw.pre] as f64;
    ui.step(0.02);
    let step = |ui: &mut crate::ui::Ui, mut f: Frame| {
        let mut g = oos_hal::sync::lock(&shared);
        (f.cfg, f.stamp) = (g.acquisition.identity().gen(), g.acquisition.identity().epoch());
        g.queue.push_back(f.into());
        drop(g);
        for _ in 0..3 {
            ui.step(0.02);
        }
    };
    step(&mut ui, pairs.clone());
    assert!(ui.meas(1, Meas::Vavg).is_nan(), "pairs alone: no mean");
    step(&mut ui, raw);
    // the raw window (10 M points) is measured by the background worker, not in the step: wait for it, bounded
    let deadline = crate::ui::lifecycle::test_wait::Wait::new("raw-window channel measurement result");
    while ui.meas(1, Meas::Vavg).is_nan() {
        deadline.ui(&ui);
        ui.step(0.02);
    }
    let v = ui.meas(1, Meas::Vavg);
    assert!((v - level).abs() < 1e-6, "Vavg {} of the raw window, level {}", v, level);
    // the pair frames after it keep the raw value
    step(&mut ui, pairs);
    assert_eq!(ui.meas(1, Meas::Vavg), v);
}

#[test]
#[ignore = "manual report: set OOS_REPORTS=1"]
// Cost of one raw deep read (deep-meas.md "Default on"): bytes, the decode in the acquisition thread and the
// screen record the UI makes of it, host times (best of 3).
fn deep_raw_read_cost_report() {
    assert_eq!(std::env::var("OOS_REPORTS").as_deref(), Ok("1"), "set OOS_REPORTS=1");
    for depth in [10_000_000u64, 100_000_000] {
        for chans in [vec![0usize], vec![0, 1]] {
            let mut m = Model {
                tscale: tscales().iter().position(|&t| (t as f64 - 1e-3).abs() < 1e-9).unwrap(),
                mdepth: mdepths().iter().position(|d| d.1 == depth).unwrap(),
                ..Default::default()
            };
            for c in 0..NCH {
                m.ch[c].on = chans.contains(&c);
            }
            let s = settings(&m, false, &Default::default());
            let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, oos_hal::dho::seq::tdiv_fs(s.delay));
            let b = batch_plan(&s, &p, 1);
            let Some(d) = deep_raw_settings(&b, &p) else {
                println!("{} {:?}: no raw window", depth, chans);
                continue;
            };
            let bytes = oos_hal::dho::seq::frame_bytes(&d, &p);
            let pair_bytes = oos_hal::dho::seq::frame_bytes(&b, &p);
            let data: Vec<u8> = (0..bytes).map(|i| (i * 7 % 251) as u8).collect();
            let best = |f: &mut dyn FnMut()| {
                (0..3)
                    .map(|_| {
                        let t = std::time::Instant::now();
                        f();
                        t.elapsed().as_secs_f64() * 1e3
                    })
                    .fold(f64::INFINITY, f64::min)
            };
            let mut fr = None;
            let dec = best(&mut || fr = Some(decode(&data, &d, &p, 1)));
            // the probe-comp square (1 kHz) with its rising edge at the trigger point, as on the unit
            let mut fr = fr.unwrap();
            let half = (0.5e-3 / fr.dt) as usize;
            let pre = fr.pre;
            for v in fr.ch.iter_mut().filter(|v| !v.is_empty()) {
                for (i, y) in v.iter_mut().enumerate() {
                    *y = if ((i + 2 * half - pre % (2 * half)) / half).is_multiple_of(2) { 2.5 } else { 0.0 }
                        + (i % 7) as f32 * 1e-3;
                }
            }
            m.trig_level = 1.25;
            let fr = std::sync::Arc::new(fr);
            let window = m.time_per_div() as f64 * HDIV as f64;
            let rec = best(&mut || {
                std::hint::black_box(crate::board::to_record(&fr, &m, 3600, window, m.delay as f64 - window / 2.0));
            });
            let trig = best(&mut || {
                std::hint::black_box(crate::board::trigger_point(&fr, &m, 3600, window, m.delay as f64 - window / 2.0));
            });
            println!("  of to_record: trigger_point {:.1} ms", trig);
            println!("{} pts {:?}: rate {} ratio {} record {:.0} ms; pair frame {:.2} MB; raw window {} pts {:.2} MB (DMA at 600 MB/s {:.0} ms); host decode {:.1} ms, to_record {:.1} ms", depth, chans, p.rate, p.ratio, p.memsel as f64 / p.rate as f64 * 1e3, pair_bytes as f64 / 1e6, d.window.unwrap().0, bytes as f64 / 1e6, bytes as f64 / 600e6 * 1e3, dec, rec);
        }
    }
}

#[test]
// Break it catches (unit 2026-11-14, CH2 Vavg `----` at 10 M, 1 ms/div): no raw window planned for two lanes (the
// trigger source CH1 is acquired with CH2), so `deepmeas::hold` waited for a raw record that never came. Every
// channel set at 10 M and 1 ms/div now gets a window that holds the screen and the trigger, within MAX_BYTES.
fn deep_raw_window_for_one_and_two_lanes_at_10m() {
    for chans in [vec![0usize], vec![1], vec![0, 1], vec![2, 3]] {
        let mut m = Model {
            tscale: tscales().iter().position(|&t| (t as f64 - 1e-3).abs() < 1e-9).unwrap(),
            mdepth: mdepths().iter().position(|d| d.1 == 10_000_000).unwrap(),
            ..Default::default()
        };
        for c in 0..NCH {
            m.ch[c].on = chans.contains(&c);
        }
        let s = settings(&m, false, &Default::default());
        let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, oos_hal::dho::seq::tdiv_fs(s.delay));
        let b = batch_plan(&s, &p, 1);
        assert!(b.compress > 1, "{:?}: a deep record through the compression", chans);
        let d = deep_raw_settings(&b, &p).unwrap_or_else(|| panic!("{:?}: no raw window (ratio {})", chans, p.ratio));
        let (n, off) = d.window.unwrap();
        let screen = (s.tdiv * 10.0 * p.rate as f64).ceil() as i64;
        // the screen around the middle of the record and the trigger point are inside, the bytes fit
        let (lo, hi) = (p.pre as i64 + off, p.pre as i64 + off + n as i64);
        let mid = p.memsel as i64 / 2;
        assert!(
            lo <= mid - screen / 2 && hi >= mid + screen / 2 && lo <= p.pre as i64 && hi > p.pre as i64,
            "{:?}: window {} + {}",
            chans,
            lo,
            n
        );
        let bytes = oos_hal::dho::seq::frame_bytes(&d, &p);
        assert!(bytes <= crate::deepmeas::MAX_BYTES, "{:?}: {} bytes, {} points x {} lanes", chans, bytes, n, p.ratio);
    }
}
