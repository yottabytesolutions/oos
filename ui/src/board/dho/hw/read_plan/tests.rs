// SPDX-License-Identifier: GPL-2.0-or-later

use super::read_plan;
use crate::board::dho::hw::{batch_plan, codes_into, raw_replay_settings, settings};
use crate::board::Frame;
use crate::model::{mdepths, Model, TIME_SCALES};
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
                writeln!(actual, "{}", golden_line(mask, us, depth, &switches)).unwrap();
            }
        }
    }
    // Run this test with OOS_UPDATE_READ_PLAN_GOLDEN=1 to replace the fixture.
    if std::env::var("OOS_UPDATE_READ_PLAN_GOLDEN").as_deref() == Ok("1") {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/board/dho/hw/read_plan/golden.txt");
        std::fs::write(path, actual).unwrap();
    } else {
        assert_eq!(actual, include_str!("golden.txt"));
    }
}

fn golden_line(mask: u32, us: u32, depth: u64, switches: &Switches) -> String {
    let tdiv = us as f32 * 1e-6;
    let mut m = Model {
        tscale: TIME_SCALES.iter().position(|&t| (t - tdiv).abs() < tdiv * 1e-6).unwrap(),
        mdepth: mdepths().iter().position(|&(_, d)| d == depth).unwrap(),
        delay: 0.0,
        trig_src: mask.trailing_zeros() as usize,
        ..Default::default()
    };
    for (i, channel) in m.ch.iter_mut().enumerate() {
        channel.on = mask & (1 << i) != 0;
    }
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
