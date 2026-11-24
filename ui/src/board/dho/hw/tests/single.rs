// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

// Break it catches: SINGLE armed for a whole batch (248 frames at 1 us/div on the unit, all but the first unseen)
#[test]
fn single_records_one_frame_per_batch() {
    for tdiv in [1e-6f32, 1e-3] {
        let mut m = Model {
            tscale: tscales().iter().position(|&t| (t - tdiv).abs() < tdiv * 1e-3).unwrap(),
            ..Default::default()
        };
        let frames = |m: &Model| {
            let s = settings(m, false, &Default::default());
            let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
            batch_plan(&s, &p, 1).frames
        };
        assert!(tdiv > 1e-4 || frames(&m) > 1, "{} s/div: RUN reads batches", tdiv);
        m.trig_mode = TrigMode::Single;
        assert_eq!(frames(&m), 1, "{} s/div", tdiv);
    }
}

// Break it catches: batches after the SINGLE frame until the UI's STOP reached the thread (`acquired` +10 at
// 1 us/div, +3 at 1 ms/div per press on the unit); or a SINGLE that waits for good where the UI refuses frames
#[test]
fn single_stops_after_the_batch_with_its_frame() {
    let mut m = Model { trig_mode: TrigMode::Single, ..Default::default() };
    let b = settings(&m, false, &Default::default());
    assert!(b.single);
    assert!(single_wants_more(&b, None, (2, 4)), "the first batch after the press");
    assert!(!single_wants_more(&b, Some((2, 4)), (2, 4)), "its frame came: no more for this press");
    let mut model = m.clone();
    let config = crate::model::config();
    let mut changes = oos_core::change::Shadow::new(&model, Default::default(), &config).unwrap();
    let mut arms = 1;
    let done = Some((changes.identity().gen(), changes.identity().epoch()));
    for _ in 0..20 {
        model.mask_src = 1 - model.mask_src;
        let id = changes.observe(&model, Default::default(), &[], &config).unwrap().identity();
        assert!(id.gen() > 0);
        arms += usize::from(single_wants_more(&b, done, (id.gen(), id.epoch())));
    }
    assert_eq!(arms, 21, "an unaccepted old-pair batch cannot complete a new pair");

    assert!(single_wants_more(&b, Some((2, 4)), (2, 5)), "the next press");
    m.trig_mode = TrigMode::Normal;
    assert!(single_wants_more(&settings(&m, false, &Default::default()), Some((2, 4)), (2, 4)), "RUN is not limited");
    // the UI may refuse the frame: protocol trigger, zone trigger
    m.trig_mode = TrigMode::Single;
    m.trig_type = TrigType::Bus;
    assert!(!settings(&m, false, &Default::default()).single);
    m.trig_type = TrigType::Edge;
    m.zones[0] = crate::zone::Zone { on: true, src: 0, inside: true, pts: vec![(0.0, 0.0), (1e-6, 1.0)] };
    assert!(m.zones[0].usable() && !settings(&m, false, &Default::default()).single);
}
