// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::{dho::hw::decode, Frame};
use crate::drawpipe::route::Route;
use crate::fastdraw::{Samples, Spans};
use crate::picture::deposit::{kernel, standard};

#[test]
fn captured_batch_008_bridges_sparse_pair_columns() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sparse-pairs/batch-008");
    let fixture = Fixture::load(&path).unwrap();
    let (settings, params) = fixture.settings();
    let (model, (width, height), _, _) = fixture.placement_view().unwrap().inputs();
    let (_, submitted) = fixture.expected_route().unwrap();
    let mut frame = Frame::default();
    let mut expected = Spans::default();
    let mut count = 0;
    for bytes in fixture.records().skip(submitted.start).take(submitted.len()) {
        let layout = decode::codes_into(bytes, settings, params, 0, &mut frame).unwrap();
        assert!(frame.envelope);
        let placed = standard::batch_plan(model, (width, height), &frame, Some((bytes, layout)), None);
        assert_eq!(placed.route, Route::Pairs);
        let (projection, trace) = placed.plan.raw().unwrap();
        let density = 0.5 / projection.x_per_sample;
        assert!((0.38..0.39).contains(&density), "pair-centred placement density: {density}");
        assert!(!projection.dots);
        for (c, span) in trace.spans.iter().enumerate() {
            let codes = layout.channel(bytes, c);
            let Some(map) = kernel::raw_map(projection, c, trace.trigger_at).filter(|_| codes.len() > 0) else {
                assert!(span.is_none(), "unexpected spans on channel {c}");
                continue;
            };
            let decoded: Vec<_> = (0..codes.len()).map(|i| codes.at(i)).collect();
            crate::compress::reference_bridged_spans(width, height, &decoded, &map, false, &mut expected);
            let span = span.as_ref().unwrap();
            assert_eq!((span.c0, &span.rows), (expected.c0, &expected.rows), "record {count}, channel {c}");
        }
        count += 1;
    }
    assert_eq!(count, 10);
    assert!(replay_route(&fixture));
}
