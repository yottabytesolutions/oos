// SPDX-License-Identifier: GPL-2.0-or-later

use super::read_plan;
use crate::board::dho::hw::{batch_plan, codes_into, raw_replay_settings, settings};
use crate::board::Frame;
use crate::model::{mdepths, AutoDepth, Model, TIME_SCALES};
use crate::sysconf::Switches;
use oos_hal::dho::seq;
use std::fmt::Write;

#[test]
fn read_plan_golden() {
    let mut actual = String::new();
    let switches = Switches { compress: true, ..Switches::default() };
    for mask in [1u32, 2, 3, 15] {
        for us in [1, 10, 100, 500, 10_000] {
            for depth in [0, 1_000_000] {
                writeln!(actual, "{}", golden_line(mask, us, depth, AutoDepth::Extended, &switches)).unwrap();
            }
        }
    }
    assert_eq!(actual, include_str!("golden.txt"));
    let mut screen = String::new();
    for mask in [1u32, 2, 3, 15] {
        for us in [1, 10, 100, 500, 10_000] {
            writeln!(screen, "{}", golden_line(mask, us, 0, AutoDepth::Screen, &switches)).unwrap();
        }
    }
    assert_eq!(screen, include_str!("screen-golden.txt"));
}

fn model_at(mask: u32, tscale: usize) -> Model {
    let mut m = Model { tscale, delay: 0.0, trig_src: mask.trailing_zeros() as usize, ..Default::default() };
    for (i, channel) in m.ch.iter_mut().enumerate() {
        channel.on = mask & (1 << i) != 0;
    }
    m
}

fn scale_at(tdiv: f32) -> usize {
    TIME_SCALES.iter().position(|&t| (t - tdiv).abs() < tdiv * 1e-6).unwrap()
}

fn golden_line(mask: u32, us: u32, depth: u64, policy: AutoDepth, switches: &Switches) -> String {
    let mut m = model_at(mask, scale_at(us as f32 * 1e-6));
    m.mdepth = mdepths().iter().position(|&(_, d)| d == depth).unwrap();
    m.auto_acquisition.set_depth(policy);
    let s = settings(&m, false, switches);
    assert_eq!(s.mask, mask);
    let (_, p) = seq::State::handover(mask, None).configure(&s, None, None).unwrap();
    let plan = read_plan(&s, &p, switches.board.zone_batch);
    let b = batch_plan(&s, &p, switches.board.zone_batch);
    assert_eq!((plan.stored, plan.frames), (&p, b.frames));
    let (n, lo) = seq::window(&b, &p);
    let k = seq::compress_k(&b, &p);
    let len = if k > 1 { seq::compressed_len(&b, &p) } else { n };
    let fb = seq::frame_bytes(&b, &p);
    let mut frame = Frame::default();
    codes_into(&[0; 18], &b, &p, 1, &mut frame).unwrap();
    let replay = plan.replay_settings(&b, true);
    assert_eq!(replay, raw_replay_settings(&b, &p, true));
    let replay = replay.map(|r| (r.window, r.compress));
    assert_eq!((lo % 8, n % 8, len % 8, fb % 16), (0, 0, 0, 0));
    assert!(len * k <= n);
    assert!(lo + n <= p.memsel);
    assert!(lo <= p.pre && p.pre < lo + n);
    format!(
        "mask={mask:#04x} us={us} depth={depth} stored={:?}/{}@{} ratio={} play=lo:{lo},n:{n},k:{k},len:{len} \
         frames={} bytes={fb} dt={:016x} pre={}/{}->{} fixed_ti={:?} replay={replay:?}",
        b.mode,
        p.memsel,
        p.rate,
        p.ratio,
        b.frames,
        frame.dt.to_bits(),
        p.pre - lo,
        k,
        frame.pre,
        frame.fixed_ti,
    )
}

#[test]
fn auto_memory_preserves_sample_rate_across_the_grid() {
    let switches = Switches::default();
    let mut shortened = 0;
    println!("s/div\tmask\tSa/s\tExtended points\tExtended s\tScreen points\tScreen s");
    for (scale, tdiv) in TIME_SCALES.into_iter().enumerate() {
        for mask in [1, 3, 15] {
            let mut m = model_at(mask, scale);
            let extended = seq::record_geometry(&settings(&m, false, &switches)).unwrap();
            m.auto_acquisition.set_depth(AutoDepth::Screen);
            let screen = seq::record_geometry(&settings(&m, false, &switches)).unwrap();
            assert_eq!(screen.rate, extended.rate, "{tdiv} s/div, mask {mask:#x}");
            assert!(screen.memsel <= extended.memsel, "{tdiv} s/div, mask {mask:#x}");
            if screen.memsel != extended.memsel {
                let tdiv_fs = seq::tdiv_fs(f64::from(tdiv));
                let on_screen =
                    (10 * tdiv_fs as u128 * u128::from(screen.rate)).div_ceil(oos_hal::dho::hz::FS as u128) as i128;
                assert_eq!(i128::from(screen.memsel), super::depth::screen_margin(on_screen));
                shortened += 1;
            }
            let span = |p: &oos_hal::dho::hz::Params| f64::from(p.memsel) / p.rate as f64;
            println!(
                "{tdiv:.3e}\t{mask}\t{}\t{}\t{:.9e}\t{}\t{:.9e}",
                extended.rate,
                extended.memsel,
                span(&extended),
                screen.memsel,
                span(&screen),
            );
        }
    }
    assert_eq!(shortened, 30);
}

#[test]
fn auto_memory_shortens_at_the_extended_divider_rate() {
    let switches = Switches::default();
    for (mask, tdiv, rate, points) in [
        (1, 10e-6, 625_000_000, 70_312),
        (1, 500e-6, 12_500_000, 70_312),
        (15, 100e-6, 62_500_000, 70_312),
        (1, 10e-3, 500_000, 56_256),
    ] {
        let mut m = model_at(mask, scale_at(tdiv));
        m.auto_acquisition.set_depth(AutoDepth::Screen);
        let s = settings(&m, false, &switches);
        let (_, p) = seq::State::handover(mask, None).configure(&s, None, None).unwrap();
        assert_eq!((p.rate, p.memsel, p.pre, p.post), (rate, points, points / 2, points / 2));
        let ordinary = oos_hal::dho::hz::prep(mask, seq::tdiv_fs(s.tdiv), i128::from(points), 0);
        assert_ne!(ordinary.rate, rate);
    }
}

#[test]
fn auto_memory_uses_the_analog_rate_with_the_logic_analyser_on() {
    let mut m = model_at(1, scale_at(500e-6));
    m.la.on = true;
    let switches = Switches::default();
    let extended = seq::record_geometry(&settings(&m, false, &switches)).unwrap();
    m.auto_acquisition.set_depth(AutoDepth::Screen);
    let screen = seq::record_geometry(&settings(&m, false, &switches)).unwrap();
    assert_eq!((extended.rate, extended.memsel), (6_250_000, 100_000));
    assert_eq!((screen.rate, screen.memsel), (6_250_000, 35_160));
}

#[test]
fn auto_memory_delay_keeps_the_trigger_and_screen_in_the_record() {
    for (delay, shorter) in [(-200e-6, false), (-60e-6, true), (60e-6, true), (200e-6, false)] {
        let mut m = model_at(1, scale_at(10e-6));
        m.delay = delay;
        let extended = settings(&m, false, &Switches::default());
        m.auto_acquisition.set_depth(AutoDepth::Screen);
        let screen = settings(&m, false, &Switches::default());
        let p = seq::record_geometry(&screen).unwrap();
        let extended = seq::record_geometry(&extended).unwrap();
        assert_eq!(p.rate, extended.rate);
        assert_eq!(p.memsel < extended.memsel, shorter);
        assert!(p.pre > 0 && p.post > 0);
        let half_screen = (5.0 * screen.tdiv * p.rate as f64).ceil() as u32;
        let center = (p.pre as f64 + screen.delay * p.rate as f64).round() as u32;
        assert!(center >= half_screen && center + half_screen <= p.memsel);
    }
}

#[test]
fn auto_memory_stop_replay_uses_the_captured_depth_and_trigger() {
    let mut m = model_at(1, scale_at(10e-6));
    m.auto_acquisition.set_depth(AutoDepth::Screen);
    let s = settings(&m, false, &Switches::default());
    let p = seq::record_geometry(&s).unwrap();
    assert_eq!((p.memsel, p.pre), (70_312, 35_156));
    for delay in [-20e-6, -0.6e-6, 0.0, 0.6e-6, 20e-6] {
        let (replay, trigger) = super::stop_plan(&s, &p, s.tdiv, delay);
        let (len, start) = seq::window(&replay, &p);
        assert!(start + len <= p.memsel);
        assert_eq!(trigger + f64::from(start), f64::from(p.pre));
    }
}
