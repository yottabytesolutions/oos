// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::board::dho::hw::window::{batch_play, configured_play_window};
use oos_hal::dho::seq;
use std::num::NonZeroU64;

const PERIOD: Duration = Duration::from_nanos(30_797_334);

#[test]
fn deep_crop_grant_fits_a_full_cycle_at_the_configured_rate() {
    let source = Duration::from_micros(1600);
    let dma = Duration::from_micros(450);
    let placement = Duration::from_micros(240);
    for (rate, capture_us, expected) in [(625_000_000, 1600, 9), (1_250_000_000, 800, 14)] {
        let settings = view(1_000_000, rate);
        let params = seq::record_geometry(&settings).unwrap();
        assert_eq!((params.memsel, params.rate), (1_000_000, rate));
        let (grant, budget) = virtual_grant(settings, &params, (source, dma, placement), (19, 11.5));
        let count = grant.records();
        let capture = Duration::from_micros(capture_us) + Duration::from_nanos(8);
        let cycle = |n| source + (capture + dma + placement) * n;
        assert_eq!(budget.as_micros(), 23_098);
        assert_eq!(count, expected);
        assert!(cycle(count) <= budget && cycle(count + 1) > budget);
        println!("1 M at {rate} Sa/s: legacy limit 19, grant {count}, cycle {:?}, budget {budget:?}", cycle(count));
    }
}

#[test]
fn auto_crop_retains_the_legacy_twenty_record_grant() {
    let settings = view(70_312, 625_000_000);
    let params = seq::record_geometry(&settings).unwrap();
    let source = Duration::from_micros(1600);
    let dma = Duration::from_micros(750);
    let placement = Duration::from_micros(200);
    let (grant, budget) = virtual_grant(settings, &params, (source, dma, placement), (20, 4.8));
    assert_eq!(grant, Grant::Overlapping(20));
    let cycle = source + (Duration::from_micros(113) + dma + placement) * 20;
    assert!(cycle <= budget);
    println!("Auto 70,312: legacy and cycle grants 20, cycle {cycle:?}, budget {budget:?}");
}

fn view(depth: i128, rate: u64) -> Settings {
    Settings { tdiv: 10e-6, depth, sample_rate: NonZeroU64::new(rate), window: None, ..Default::default() }
}

fn virtual_grant(
    mut settings: Settings,
    params: &hz::Params,
    (fixed, dma, placement): (Duration, Duration, Duration),
    (legacy, pre): (u32, f32),
) -> (Grant, Duration) {
    let mut cap = crate::pacing::FrameCap::default();
    // Feed virtual phase durations. The legacy cap treats fill as a flat 6 ms allowance.
    let read = (fixed + dma * legacy).as_secs_f32() * 1e3;
    cap.done(legacy, read, pre, 100, false, || PERIOD);
    assert_eq!(cap.apply(100), legacy);
    cap.observe_window(legacy, fixed, dma * legacy);
    let out = crate::board::HwShared::default();
    let id = Identity::default();
    {
        let mut shared = oos_hal::sync::lock(&out);
        shared.record_flow.cost.set_identity(id);
        shared.record_flow.cost.observe(id, 1, placement, Duration::ZERO);
    }
    settings.compress = crate::compress::k_for(params.rate, settings.tdiv, params.memsel);
    let window = configured_play_window(&settings, params, 1024);
    assert!(window.is_some());
    let b = batch_play(&settings, params, 1, window);
    let grant = grant((None, b.frames), &cap, (&out, id), (&b, params), (WORK_BUDGET, || PERIOD));
    (grant, crate::drawpipe::budget::pipeline_budget(PERIOD))
}
