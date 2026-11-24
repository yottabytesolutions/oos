// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[test]
fn depth_stays_within_the_limit_of_the_channels_on() {
    let mut m = Model {
        mdepth: mdepths().iter().position(|d| d.1 == 125_000_000).unwrap(),
        tscale: tscales().iter().position(|&t| t >= 1e-3).unwrap(),
        ..Default::default()
    };
    for c in 0..NCH {
        m.ch[c].on = c == 0;
    }
    assert_eq!(settings(&m, false, &Default::default()).depth, 125_000_000);
    for c in 0..NCH {
        m.ch[c].on = true;
    }
    assert_eq!(settings(&m, false, &Default::default()).depth, m.max_mem() as i128);
    assert_eq!(m.max_mem(), 31_250_000);
}

// Break it catches: hardware segments requested in roll or with the LA, a read-back batch larger than the UI queue
// or without the header that carries the stamp.
#[test]
fn hardware_segments_only_where_they_record() {
    let mut m = Model {
        tscale: tscales().iter().position(|&x| (x as f64 - 1e-7).abs() < 1e-10).unwrap(),
        ..Default::default()
    };
    (m.segments, m.seg_hw, m.seg_ivl) = (500, true, 0.0);
    let s = settings(&m, false, &Default::default());
    assert_eq!(s.segments, Some(Segments { n: 500, interval: 0.0 }));
    assert_eq!(seg_max(&m), Some(61_440));
    m.seg_hw = false;
    assert_eq!(settings(&m, false, &Default::default()).segments, None);
    m.seg_hw = true;
    m.la.on = true;
    assert_eq!((settings(&m, false, &Default::default()).segments, seg_max(&m)), (None, None));
    m.la.on = false;
    m.tscale = tscales().iter().position(|&x| x >= 0.2).unwrap();
    assert_eq!(settings(&m, false, &Default::default()).segments, None);
    // read-back: header on, at most half the queue, ~4 MB
    for t in [1e-7, 1e-5, 1e-3] {
        m.tscale = tscales().iter().position(|&x| (x as f64 - t).abs() < t * 1e-3).unwrap();
        let s = settings(&m, false, &Default::default());
        let p = oos_hal::dho::seq::State::handover(s.mask, None).configure(&s, None, None).unwrap().1;
        let r = seg_read_plan(&s, &p);
        let fb = oos_hal::dho::seq::frame_bytes(&r, &p);
        assert!(r.header && r.frames >= 1 && r.frames as usize <= crate::board::QUEUE_MAX / 2);
        assert!(r.frames == 1 || r.frames as usize * fb <= 4_000_000, "{} x {}", r.frames, fb);
    }
}

#[test]
fn settings_from_the_model() {
    let mut m = Model::default();
    m.ch[1].on = true;
    let s = settings(&m, false, &Default::default());
    assert_eq!(s.mask, 3);
    assert_eq!(s.trig_src, 1);
    // 200 us/div with 2 channels: Auto would be 1.25 M points -> live depth 100 k
    assert_eq!(s.depth, LIVE_DEPTH);
    assert!(s.window.is_none());
    // edge: the level itself, even below level 2; runt: upper level and lower level
    m.trig_level = 0.2;
    assert_eq!(settings(&m, false, &Default::default()).level, 0.2f32 as f64);
    m.trig_type = TrigType::Runt;
    m.trig_level2 = 1.8;
    let s = settings(&m, false, &Default::default());
    assert_eq!(s.level, 1.8f32 as f64);
    assert!(matches!(s.trigger, Trig::Runt { lo, .. } if (lo - 0.2).abs() < 1e-6));
}

// Break it catches: a live batch whose frames the FPGA pads in the stream (frame bytes not a multiple of 8), so
// every read moves the whole batch once more in `seq::unpad`. All time bases, depths and channel sets of the UI
// that read through the batch path (not roll or scan); with and without the LA.
#[test]
fn live_batches_need_no_stream_padding() {
    let mut m = Model::default();
    let (mut bad, mut checked, mut la_seen) = (Vec::new(), 0, false);
    for la in [false, true] {
        m.la.on = la;
        for chans in 1u32..16 {
            for c in 0..NCH {
                m.ch[c].on = chans & (1 << c) != 0;
            }
            for t in 0..tscales().len() {
                m.tscale = t;
                if slow_mode(&m) != Slow::Off {
                    continue;
                }
                for d in 0..mdepths().len() {
                    m.mdepth = d;
                    let s = settings(&m, false, &Default::default());
                    let Ok((_, p)) = oos_hal::dho::seq::State::handover(s.mask, None).configure(&s, None, None) else {
                        continue;
                    };
                    let b = batch_plan(&s, &p, 1);
                    let fb = oos_hal::dho::seq::frame_bytes(&b, &p);
                    checked += 1;
                    la_seen |= s.la.is_some();
                    if !fb.is_multiple_of(8) {
                        bad.push((la, chans, tscales()[t], mdepths()[d].0, fb));
                    }
                }
            }
        }
    }
    assert!(checked > 1000 && la_seen, "{} set-ups checked, LA among them: {}", checked, la_seen);
    assert!(bad.is_empty(), "{} padded set-ups, e.g. {:?}", bad.len(), &bad[..bad.len().min(8)]);
}
