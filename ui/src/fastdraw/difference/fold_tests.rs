// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

/// Spans of random heights in random columns (some saturating), folded by rows in bands: the same map as the
/// first fold column by column, for several row ranges, band widths and widths that are no multiple of them.
#[test]
fn fold_by_rows_equals_fold_by_columns() {
    let mut seed = 12345u64;
    let mut rnd = |n: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % n
    };
    for &(w, h, lo, hi, bc) in &[
        (1000usize, 480usize, 0usize, 480usize, 512usize),
        (1000, 480, 0, 480, 1000),
        (77, 50, 3, 41, 20),
        (64, 64, 0, 64, 1),
        (33, 31, 30, 31, 32),
    ] {
        let hp = h + 1;
        let (mut acc, mut cols) = (vec![0u16; w * hp], vec![0u16; w * hp]);
        for _ in 0..w * 3 {
            let c = rnd(w);
            let r0 = lo + rnd(hi - lo);
            let r1 = r0 + rnd(hi - r0);
            let wgt = if rnd(10) == 0 { 40000 } else { 1 + rnd(300) as u16 };
            let (b, x) = (c / bc, c % bc);
            let bw = bc.min(w - b * bc);
            diff_span(&mut acc[b * bc * hp..][..bw * hp], bw, x, r0, r1, wgt);
            diff_span(&mut cols[c * hp..][..hp], 1, 0, r0, r1, wgt);
        }
        let mut base = vec![0u16; w * h];
        for x in base.iter_mut() {
            *x = rnd(3000) as u16;
        }
        let (mut m1, mut m2) = (base.clone(), base);
        fold_t(&mut m1, &mut acc, w, h, bc, lo..hi);
        fold_t_cols(&mut m2, &mut cols, w, h, lo..hi);
        assert!(m1 == m2, "{w}x{h} rows {lo}..{hi} bands of {bc}");
        assert!(acc.iter().all(|&x| x == 0) && cols.iter().all(|&x| x == 0));
    }
}

/// Time of a fold of a full map of tall spans (cargo test --release fold_speed -- --ignored --nocapture).
#[test]
#[ignore = "host timing: set OOS_PERF_TESTS=1"]
fn fold_speed() {
    assert_eq!(std::env::var("OOS_PERF_TESTS").as_deref(), Ok("1"), "set OOS_PERF_TESTS=1");
    let (w, h, bc) = (1024usize, 456usize, 512usize);
    let hp = h + 1;
    let span = |c: usize, k: usize| ((k * 7 + c) % 200, 250 + (k * 11 + c) % 200);
    let time = |f: &mut dyn FnMut(&mut [u16], &mut [u16]), fill: &dyn Fn(&mut [u16])| {
        let (mut map, mut acc, mut best) = (vec![0u16; w * h], vec![0u16; w * hp], f64::MAX);
        for _ in 0..30 {
            fill(&mut acc);
            let t = std::time::Instant::now();
            f(&mut map, &mut acc);
            best = best.min(t.elapsed().as_secs_f64());
        }
        best * 1e3
    };
    let rows = time(&mut |m, a| fold_t(m, a, w, h, bc, 0..h), &|acc| {
        for c in 0..w {
            let (b, x) = (c / bc, c % bc);
            for k in 0..20 {
                let (r0, r1) = span(c, k);
                diff_span(&mut acc[b * bc * hp..][..bc * hp], bc, x, r0, r1, 3);
            }
        }
    });
    let cols = time(&mut |m, a| fold_t_cols(m, a, w, h, 0..h), &|acc| {
        for c in 0..w {
            for k in 0..20 {
                let (r0, r1) = span(c, k);
                diff_span(&mut acc[c * hp..][..hp], 1, 0, r0, r1, 3);
            }
        }
    });
    println!("fold rows {rows:.3} ms, columns {cols:.3} ms");
}
