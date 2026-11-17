// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! NMRA DCC against generated signals with known content.

use super::*;

fn dcc_frames() -> Vec<DccFrame> {
    vec![
        DccFrame { address: 0x03, data: vec![0x3F], xor: None, preamble: 14 },
        DccFrame { address: 0x28, data: vec![0x00, 0x28], xor: None, preamble: 12 },
    ]
}

fn dcc_want(fr: &[DccFrame]) -> Vec<(Kind, u32, bool)> {
    let mut want = Vec::new();
    for f in fr {
        let mut payload = vec![f.address];
        payload.extend_from_slice(&f.data);
        let x = f.xor.unwrap_or_else(|| dcc_xor(&payload));
        want.push((F("PRE"), f.preamble.max(10), true));
        want.push((ID, f.address as u32, true));
        for &d in &f.data {
            want.push((Kind::Data, d as u32, true));
        }
        want.push((Kind::Crc, x as u32, f.xor.is_none_or(|g| g == dcc_xor(&payload))));
    }
    want
}

#[test]
fn dcc_xor_and_baseline_packets() {
    assert_eq!(dcc_xor(&[0x03, 0x3F]), 0x3C);
    assert_eq!(dcc_xor(&[0x28, 0x00, 0x28]), 0x00);
    let fr = dcc_frames();
    let dt = DCC_ONE_HALF / 8.0;
    let mut v = wave(&gen::dcc(&fr, 0.0, 3.3), dt, 200e-6);
    add_noise(&mut v, 0.05, 9);
    let p = dcc(&v, dt, 1.65, 0.3);
    assert_eq!(kv(&p), dcc_want(&fr));
    // nothing on a flat line or with too few samples per half-bit
    assert!(dcc(&vec![1.65; 5000], dt, 1.65, 0.3).is_empty());
    assert!(dcc(&v, 50e-6, 1.65, 0.3).is_empty());
}

#[test]
fn dcc_bad_xor_and_cut_packet() {
    let mut fr = dcc_frames();
    fr[0].xor = Some(0x3C ^ 0xFF);
    let dt = DCC_ONE_HALF / 8.0;
    let v = wave(&gen::dcc(&fr, 0.0, 3.3), dt, 200e-6);
    let p = dcc(&v, dt, 1.65, 0.3);
    let crcs: Vec<(u32, bool)> = p.iter().filter(|x| x.kind == Kind::Crc).map(|x| (x.value, x.ok)).collect();
    assert_eq!(crcs, [(0x3C ^ 0xFF, false), (0x00, true)]);
    // a record that ends inside the first packet: no partial decode
    let full = wave(&gen::dcc(&dcc_frames()[..1], 0.0, 3.3), dt, 200e-6);
    let cut = &full[..full.len() / 2];
    assert!(dcc(cut, dt, 1.65, 0.3).is_empty());
}
