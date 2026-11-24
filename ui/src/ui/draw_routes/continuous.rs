// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::Frame;
use crate::drawpipe::route::Route as Placement;

#[test]
fn admitted_modes_use_their_pool_adapters_and_keep_the_newest_record() {
    for (mode, tdiv, kind, expected) in [
        (AcqMode::Normal, 1e-6, BatchKind::Codes, Placement::Dense),
        (AcqMode::Peak, 1e-6, BatchKind::Codes, Placement::Peak),
        (AcqMode::HiRes, 10e-6, BatchKind::Codes, Placement::Boxcar),
        (AcqMode::HiRes, 1e-6, BatchKind::Codes, Placement::Upsampled),
        (AcqMode::Normal, 20e-9, BatchKind::Codes, Placement::Upsampled),
        (AcqMode::Normal, 200e-6, BatchKind::LivePairs, Placement::Pairs),
        (AcqMode::Normal, 1e-3, BatchKind::DeepPairs, Placement::Pairs),
    ] {
        let _scope = Scope::new();
        let mut route = Route::new(kind, true, 2, tdiv);
        route.ui.m.acq = mode;
        assert!(route.ui.fast_ok());
        let mut reference = scripted::source(kind, 123);
        let batch = reference.next_batch(&route.ui.m, route.ui.switches(), 3, None).unwrap();
        let mut want = Phosphor::new(route.ui.ph.w, route.ui.ph.h);
        let frames: [Frame; 3] = std::array::from_fn(|index| batch.frame(index, 0));
        let expected_record = crate::wave::hw_record(&route.ui.m, Arc::new(batch.newest(0)));
        let (codes, fb) = (batch.codes(), batch.frame_bytes());
        let bytes = batch.into_bytes();
        for (index, frame) in frames.iter().enumerate() {
            if expected == Placement::Upsampled {
                let raw = &bytes.bytes()[index * fb..(index + 1) * fb];
                let layout = codes.as_ref().unwrap()(index, raw, &mut Frame::default()).unwrap();
                drawpipe::stage::fixture_bench::add_short_columns(&mut want, &route.ui.m, frame, raw, layout);
            } else {
                drawpipe::draw_frame(&mut want, &route.ui.m, frame, crate::sparse::Interp::Exact, None);
            }
        }
        let before = drawpipe::counters().routes.count(expected);
        route.step(3);
        assert_eq!(drawpipe::counters().routes.count(expected) - before, 3, "{mode:?} {kind:?}");
        assert_eq!(route.ui.wfm_total, 3);
        assert!(route.ui.ph.maps() == want.maps(), "{mode:?} {kind:?} {tdiv}");
        assert!(want.maps()[0].iter().any(|&count| count > 0));
        let bits = |record: &Record| record.ch.each_ref().map(|ch| ch.iter().map(|v| v.to_bits()).collect::<Vec<_>>());
        assert_eq!(bits(route.ui.last[0].as_ref().unwrap()), bits(&expected_record), "{mode:?} {kind:?}");
    }
}

#[test]
fn only_native_trigger_conditions_are_eligible_for_continuous_placement() {
    let mut route = Route::new(BatchKind::Codes, true, 1, 1e-6);
    for kind in TRIG_TYPES {
        route.ui.m.trig_type = kind;
        route.ui.m.video_sync = VSync::Odd;
        assert_eq!(route.ui.fast_ok(), kind != TrigType::Bus, "{kind:?}");
    }
    route.ui.m.trig_type = TrigType::Video;
    route.ui.m.video_sync = VSync::AllFields;
    assert!(!route.ui.fast_ok());

    route.ui.m.trig_type = TrigType::Edge;
    route.ui.m.deskew_ch[0] = 2e-9;
    for mode in [AcqMode::Peak, AcqMode::HiRes] {
        route.ui.m.acq = mode;
        assert!(!route.ui.fast_ok(), "{mode:?} uses a shifted record projection");
    }
}
