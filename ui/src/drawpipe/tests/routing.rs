// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

// A frame must stay in the near window under AC or LF
// reject; an edge trigger on an exact `pre`), on the decoded frame or from the words; a far edge no longer found
// where the wide search must stay (decimated edge reads); a DC refinement
// that differs from `trignear::edge_smooth_near` (the refinement before the near-only paths).
#[test]
fn near_only_where_the_hardware_point_stands_wide_elsewhere() {
    let (w, h) = (970, 448);
    // -1 V up to 10 k samples after the hardware point (50 k), then +1 V: the only rising edge is far away
    let far = |_k: usize, _c: usize, i: usize| if i < 60_000 { -1.0 } else { 1.0 } + 0.002 * ((i * 7919) % 13) as f64;
    let (bytes, fb, s, p) = frames_of(1, 100_000, 1, &far);
    let f = crate::board::dho::hw::decode(&bytes, &s, &p, 0);
    let mut g0 = Frame::default();
    let lay = crate::board::dho::hw::codes_into(&bytes, &s, &p, 0, &mut g0).expect("raw frame");
    assert!(
        f.pre == 50_000 && f.trigger_anchor.at_adc_rate() && g0.trigger_anchor.at_adc_rate(),
        "a full-rate raw read is exact"
    );
    // at the hardware point: a level no sample reaches finds no edge
    let mut at_pre = Phosphor::new(w, h);
    let mut m = model(1e-5, false, false);
    m.trig_level = 5.0;
    assert!(!draw_frame(&mut at_pre, &m, &f, Interp::Exact, None));
    let cases: [Case; 10] = [
        ("dc exact", TrigCoup::Dc, true, |_| {}, true),
        ("hf reject exact", TrigCoup::HfReject, true, |_| {}, true),
        ("ac", TrigCoup::Ac, false, |_| {}, true),
        ("lf reject", TrigCoup::LfReject, false, |_| {}, true),
        ("dc decimated", TrigCoup::Dc, false, |_| {}, false),
        ("dc decimated noise reject", TrigCoup::Dc, false, |m| m.trig_nrej = true, false),
        ("hf reject decimated", TrigCoup::HfReject, false, |_| {}, false),
        ("dc noise reject exact", TrigCoup::Dc, true, |m| m.trig_nrej = true, true),
        ("dc holdoff exact", TrigCoup::Dc, true, |m| m.holdoff = 1e-6, true),
        ("pulse", TrigCoup::Dc, true, |m| m.trig_type = TrigType::Pulse, false),
    ];
    for case in cases {
        check_trigger_route(case, (&bytes, fb, &s, &p), (&f, &g0, lay), &at_pre);
    }
    verify_exact_read_flags(&bytes, &s, p);
}

type Case = (&'static str, TrigCoup, bool, fn(&mut Model), bool);

fn check_trigger_route(
    (name, coup, exact, set, near): Case,
    (bytes, fb, s, p): (&[u8], usize, &Settings, &hz::Params),
    (f, g0, lay): (&Frame, &Frame, CodeLayout),
    at_pre: &Phosphor,
) {
    let (w, h) = (at_pre.w, at_pre.h);
    let mut m = model(1e-5, false, false);
    m.trig_coup = coup;
    set(&mut m);
    let mut fe = f.clone();
    fe.trigger_anchor = crate::board::TriggerAnchor::new(exact);
    let (mut a, mut b) = (Phosphor::new(w, h), Phosphor::new(w, h));
    let (mut sa, mut sb) = (([0u64; 4], (0.0, 0.0)), ([0u64; 4], (0.0, 0.0)));
    let ta = draw_frame(&mut a, &m, &fe, Interp::Exact, Some(Stats { edges: &mut sa.0, off: &mut sa.1 }));
    let mut meta = g0.clone();
    meta.trigger_anchor = crate::board::TriggerAnchor::new(exact);
    let codes = Arc::new(move |_, _: &[u8], f: &mut Frame| {
        f.clone_from(&meta);
        Some(lay)
    });
    let pipe = Pipe::start(1, 0, fresh());
    pipe.set_view(Some(&m), w, h, Interp::Exact);
    pipe.feed_codes(Box::new(Bytes(bytes.to_vec())), fb, 1, 0, dec(s, p), Some(codes));
    settle(&pipe);
    let accepted = pipe.accept(&mut b, &mut false, &mut sb.0, &mut sb.1);
    assert_eq!((accepted.frames, accepted.triggered), (1, u64::from(ta)), "{name}");
    assert!(a.maps()[0] == b.maps()[0] && sa == sb, "{name}: codes path differs");
    if m.trig_type == TrigType::Pulse {
        assert_eq!(sa, ([0; 4], (0.0, 0.0)));
        assert!(a.maps()[0] == at_pre.maps()[0], "{name}: hardware event stays at pre");
    } else if near {
        assert_eq!(sa.0, [0, 0, 0, 1], "{}", name);
        assert!(a.maps()[0] == at_pre.maps()[0], "{}: not drawn at the hardware point", name);
    } else {
        assert_eq!(sa.0, [0, 0, 1, 0], "{}", name);
        assert!((sa.1 .1 - 10_000.0).abs() < 2.0, "{}: offset {:?}", name, sa.1);
        assert!(a.maps()[0] != at_pre.maps()[0], "{}", name);
    }
}

fn verify_exact_read_flags(bytes: &[u8], s: &Settings, p: hz::Params) {
    // a read at the full rate is exact, compressed too (to its pair); a decimated one is not, compressed or not
    for (rate, compress, exact) in [(1_250_000_000u64, 4u32, true), (625_000_000, 1, false), (625_000_000, 4, false)] {
        let q = hz::Params { rate, ..p };
        let t = Settings { compress, ..s.clone() };
        assert_eq!(
            crate::board::dho::hw::decode(bytes, &t, &q, 0).trigger_anchor.at_adc_rate(),
            exact,
            "rate {} compress {}",
            rate,
            compress
        );
    }
}

// Break it catches: a compressed read (min/max pairs of the live compression) at the full rate whose search still
// runs wide, or moves to an edge outside the near window; a decimated compressed read that no longer searches
// wide; a near window (in points) that does not cover the pair of the hardware point at the largest k the full
// rate can have (a record of at most `compress::MAX_DEPTH` points).
#[test]
fn compressed_full_rate_reads_search_near_decimated_ones_wide() {
    let (w, h) = (970, 448);
    verify_pair_near_window(w);
    // a compressed frame: pairs (min, max) of a step from -1 to +1 V, the only rising edge 2 k points after the
    // hardware point (far outside the near window), or in the point's pair
    let n = 20_000usize;
    let pairs = |edge_at: usize| -> Vec<f32> {
        (0..n)
            .flat_map(|j| {
                let base = if j >= edge_at { 1.0 } else { -1.0 };
                [base - 0.01, base + 0.01]
            })
            .collect()
    };
    let mut m = model(1e-5, false, false);
    m.trig_level = 0.0;
    let dt = 1e-5 * HDIV as f64 / (2 * n) as f64;
    let frame = |v: Vec<f32>, exact: bool| Frame {
        dt,
        pre: n,
        ch: [v, Vec::new(), Vec::new(), Vec::new()],
        envelope: true,
        acq: (1.25e9, 1_000_000),
        trigger_anchor: crate::board::TriggerAnchor::new(exact),
        ..Default::default()
    };
    assert!(crate::compress::live(&frame(Vec::new(), true)));
    for (exact, near) in [(true, true), (false, false)] {
        let f = frame(pairs(n / 2 + 1000), exact);
        let mut ph = Phosphor::new(w, h);
        let mut st = ([0u64; 4], (0.0, 0.0));
        draw_frame(&mut ph, &m, &f, Interp::Exact, Some(Stats { edges: &mut st.0, off: &mut st.1 }));
        let want = if near { [0, 0, 0, 1] } else { [0, 0, 1, 0] };
        assert_eq!(st.0, want, "exact {}: {:?}", exact, st);
        // the edge in the pair of the hardware point is found either way
        let f = frame(pairs(n / 2), exact);
        let mut st = ([0u64; 4], (0.0, 0.0));
        draw_frame(&mut ph, &m, &f, Interp::Exact, Some(Stats { edges: &mut st.0, off: &mut st.1 }));
        assert_eq!(st.0, [1, 0, 0, 0], "exact {}: edge at the point", exact);
    }
}

fn verify_pair_near_window(w: usize) {
    // the largest k at the full rate, over every time base and channel ratio, and the near window there
    let mut worst = (0u32, usize::MAX);
    for ratio in [1u64, 2, 4] {
        let rate = 1_250_000_000 / ratio;
        for &t in tscales().iter() {
            let memsel = (rate as f64 * t as f64 * HDIV as f64).round();
            if memsel > crate::compress::MAX_DEPTH as f64 {
                continue;
            }
            let k = crate::compress::k_for(rate, t as f64, memsel as u32);
            if k < 2 {
                continue;
            }
            let x_per_point = k as f64 / rate as f64 / t as f64 * w as f64 / HDIV as f64;
            let col = crate::compress::smooth_len(x_per_point);
            let near = crate::trignear::NEAR_COLS * col + crate::trignear::NEAR_PAD;
            if k > worst.0 || (k == worst.0 && near < worst.1) {
                worst = (k, near);
            }
        }
    }
    println!("largest k at the full rate {}, near window there {} points", worst.0, worst.1);
    assert!(worst.0 >= 2, "no compressed full-rate read");
    assert!(
        worst.1 >= 8 * crate::compress::PAIR,
        "k {}: near window {} points against a pair of {}",
        worst.0,
        worst.1,
        crate::compress::PAIR
    );
}
