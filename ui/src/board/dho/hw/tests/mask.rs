// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// Live compression and the mask test never run together: a mask (in software or in the FPGA, which needs
/// `mask_on` too) keeps every frame raw, so the fast path the hardware mask keeps on draws uncompressed frames.
#[test]
fn a_mask_keeps_live_compression_off() {
    let mut m = Model {
        dots: DotsMode::Off,
        tscale: tscales().iter().position(|&t| (t as f64 - 10e-6).abs() < 1e-11).unwrap(),
        ..Default::default()
    };
    // dots keep frames raw (compress::allowed)
    let mut s = settings(&m, false, &Default::default());
    live_compress(&m, &mut s);
    assert!(s.compress > 1, "compression applies at 10 us/div without a mask");
    m.mask_on = true;
    let mut s = settings(&m, false, &Default::default());
    live_compress(&m, &mut s);
    assert_eq!(s.compress, 1);
}

/// The zones in the hardware tables, and the "touched" logic modelled on the read-out words (the assumed sample
/// scale): agrees with the software zone test on boxes on, off and across a square wave's edge.
#[test]
fn hardware_zone_tables_agree_with_the_software_zones() {
    use oos_hal::dho::{mask, zone};
    let (n, rate, pre, scale, offset) = (20_000u32, 100_000_000u64, 10_000u32, 1.0, -1.5);
    let dt = 1.0 / rate as f64;
    // 0 / 3 V square, period 4000 samples (40 us), rising edge at the trigger (sample 10,000)
    let v: Vec<f32> = (0..n as usize).map(|k| if (k / 2000) % 2 == 0 { 0.0 } else { 3.0 }).collect();
    let t0 = -(pre as f64) * dt;
    let touched = |poly: &[(f64, f64)]| -> bool {
        let t = zone::table(
            &zone::Zone { poly: poly.to_vec(), inside: true },
            n,
            rate,
            pre,
            1,
            scale,
            offset,
            1.0,
            &mask::Map::default(),
        );
        let per = mask::Map::default().layout(n, 1).1 as usize;
        let code = |x: f32| ((32768.0 + (x as f64 + offset) / scale * 7500.0).round() as u32 >> mask::SHIFT) as u8;
        let mut prev: Option<u8> = None;
        for (k, &x) in v.iter().enumerate() {
            let c = t[(k / per).min(t.len() - 1)];
            let s = code(x);
            if c.on
                && ((c.lo..=c.hi).contains(&s)
                    || prev.is_some_and(|p| (p < c.lo && s > c.hi) || (p > c.hi && s < c.lo)))
            {
                return true;
            }
            prev = Some(s);
        }
        false
    };
    // (box, touched): on the high level, on the low level, above the trace, across the rising edge at t = 0
    let cases = [
        (vec![(2e-6, 2.5), (8e-6, 3.5)], true),
        (vec![(-8e-6, -0.5), (-2e-6, 0.5)], true),
        (vec![(2e-6, 3.6), (8e-6, 4.0)], false),
        (vec![(-1e-6, 1.0), (1e-6, 2.0)], true),
        (vec![(5e-6, 1.0), (15e-6, 2.0)], false),
    ];
    for (pts, want) in cases {
        let sw = crate::zone::hits(&crate::zone::Zone { on: true, src: 0, inside: true, pts: pts.clone() }, &v, t0, dt);
        assert_eq!((sw, touched(&pts)), (want, want), "{:?}", pts);
    }
}
