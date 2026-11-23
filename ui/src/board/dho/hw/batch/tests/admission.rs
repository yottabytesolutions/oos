// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::picture::deposit::admission::Share;
use std::sync::Arc;
use std::time::Duration;

fn fund(pipe: &Pipe, bytes: usize, entries: usize) {
    pipe.fund_taps(Share::checked(bytes, 0, bytes, entries, Arc::new(())).unwrap(), Duration::from_millis(1)).unwrap();
}

#[test]
fn checked_share_and_unfunded_board_plans_fail_closed() {
    assert!(Share::checked(100, 51, 50, 1, Arc::new(())).is_none());
    assert!(Share::checked(usize::MAX, usize::MAX, 1, 1, Arc::new(())).is_none());
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (m, sw, b, p) = normal_lines_plan(10e-6, true);
    let pipe = Pipe::start(1, 0, crate::drawpipe::counters());
    let id = Identity::default();
    pipe.publish_view(&m, 970, 448, sw.interp, id);
    assert!(!tap::admit(&pipe, &b, &p, id));
    fund(&pipe, 1 << 20, 256);
    assert!(!tap::admit(&pipe, &b, &p, id), "mandatory raw replay cannot fit");
    assert!(pipe.tap_start(id, Instant::now()).is_none());
}

#[test]
fn admitted_board_bounds_pacing_and_warm_lookup() {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (m, sw, mut b, p) = normal_lines_plan(10e-6, true);
    let pipe = Pipe::start(1, 0, crate::drawpipe::counters());
    let id = Identity::default();
    pipe.publish_view(&m, 970, 448, sw.interp, id);
    fund(&pipe, 64 << 20, 256);
    assert!(tap::admit(&pipe, &b, &p, id));
    assert_eq!(pipe.tap_drain(), 37);
    let (allocs, admitted) = crate::system::test_allocator::count_own(|| tap::admit(&pipe, &b, &p, id));
    assert!(admitted);
    assert_eq!(allocs, 0);
    drop(pipe.reserve_plan(id).unwrap());
    let (allocs, reservation) = crate::system::test_allocator::count_own(|| pipe.reserve_plan(id));
    assert_eq!(allocs, 1, "only the shared grant token allocates, no waveform vectors");
    drop(reservation.unwrap());
    let source = tap::source_reservation(Some(&pipe), id, || false).unwrap().unwrap();
    drop(source);
    let now = Instant::now() + Duration::from_secs(2);
    assert_eq!(pipe.tap_start(id, now), Some(Duration::ZERO));
    assert_eq!(pipe.tap_start(id, now), Some(Duration::from_millis(1)));
    assert_eq!(pipe.tap_start(id, now + Duration::from_secs(1)), Some(Duration::ZERO));
    assert_eq!(pipe.tap_start(id, now + Duration::from_secs(1)), Some(Duration::from_millis(1)));
    let oversized = Frame { ch: [vec![0.0; 1_000_000], vec![], vec![], vec![]], ..Default::default() };
    assert!(pipe.reserve_waveform(&oversized, id).is_none());
    b.la = Some(Default::default());
    assert!(!tap::admit(&pipe, &b, &p, id));
}

#[test]
fn retained_frame_keeps_old_plan_charged_across_format_replacement() {
    retaining_overlap(64 << 20, 74);
    retaining_overlap(40 << 20, 256);
}

fn retaining_overlap(bytes: usize, entries: usize) {
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (m, sw, b, p) = normal_lines_plan(10e-6, true);
    let pipe = Pipe::start(1, 0, crate::drawpipe::counters());
    let id = Identity::default();
    pipe.publish_view(&m, 970, 448, sw.interp, id);
    fund(&pipe, bytes, entries);
    assert!(tap::admit(&pipe, &b, &p, id));
    let drain = pipe.tap_drain();
    let raw = oos_hal::dho::seq::raw_settings(&b);
    let mut frame = tap::storage(&raw, oos_hal::dho::seq::frame_bytes(&raw, &p)).unwrap();
    let charge = pipe.reserve_plan(id).unwrap();
    for ch in &mut frame.ch {
        ch.resize(ch.capacity(), 0.0);
    }
    frame.dt = 1e-9;
    frame.basic.retain(charge.clone());
    let retained = Arc::new(frame);
    drop(charge);
    assert!(!tap::admit(&pipe, &raw, &p, id), "old and new FIFO grants overlap");
    assert_eq!(pipe.tap_drain(), drain, "old reader retains its drain contract");
    let result = crate::board::QueuedFrame::new(retained.clone(), crate::board::PictureEligibility::Pending)
        .read(|f| crate::wave::hw_record(&m, f));
    assert!(result.raw.is_some());
    drop(retained);
    let config = crate::model::config();
    let mut shadow = oos_core::change::Shadow::new(&m, Default::default(), &config).unwrap();
    let current =
        shadow.observe(&m, Default::default(), &[oos_core::change::Request::Clear], &config).unwrap().identity();
    pipe.publish_view(&m, 970, 448, sw.interp, current);
    assert!(pipe.reserve_waveform(&result.raw.as_ref().unwrap().frame, id).is_none());
    assert!(!tap::admit(&pipe, &raw, &p, current), "old identity still funds the retaining record");
    drop(result);
    assert!(tap::admit(&pipe, &raw, &p, current));
}

#[test]
fn application_profile_drives_real_board_admission_and_source_pacing() {
    for configured in [false, true] {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let conf = if configured { crate::drawpipe::funding::TEST_PROFILE } else { "" };
        let sw = crate::sysconf::Switches::parse(&|_| None, conf, "", Default::default());
        let pipe = crate::drawpipe::start_pipe(&sw);
        let id = Identity::default();
        for compress in [false, true] {
            for tdiv in [1e-6, 10e-6] {
                let (m, geometry, b, p) = normal_lines_plan(tdiv, compress);
                pipe.publish_view(&m, 970, 448, geometry.interp, id);
                assert!(deposit_plan(&b, &p, id, true).0.is_some());
                if configured {
                    assert_eq!(pipe.tap_drain(), 70);
                    for records in [1, 2] {
                        println!("F2_CONFIGURED tdiv={tdiv} compress={compress} records={records} min_cycle_ns=500000 B={} capacity_rate={} physical=unqualified", pipe.tap_drain(), records * 2000);
                    }
                }
            }
        }
        if configured {
            let now = Instant::now() + Duration::from_secs(2);
            assert_eq!(pipe.tap_start(id, now), Some(Duration::ZERO));
            assert_eq!(pipe.tap_start(id, now), Some(Duration::from_micros(500)));
            assert_eq!(pipe.tap_start(id, now + Duration::from_secs(1)), Some(Duration::ZERO));
            assert!(pipe.has_funded_taps(id));
        } else {
            assert!(tap::source_reservation(Some(pipe), id, || false).unwrap().is_none());
            assert!(!pipe.has_funded_taps(id));
        }
    }
}

#[test]
fn application_memory_boundary_exposes_fastest_one_and_two_record_capacity() {
    use crate::picture::deposit::admission::Funding;
    for compress in [false, true] {
        for tdiv in [1e-6, 10e-6] {
            let (_, _, b, p) = normal_lines_plan(tdiv, compress);
            let raw = oos_hal::dho::seq::raw_settings(&b);
            let maximum = [&b, &raw]
                .into_iter()
                .map(|s| {
                    tap::storage(s, oos_hal::dho::seq::frame_bytes(s, &p))
                        .unwrap()
                        .ch
                        .iter()
                        .map(Vec::capacity)
                        .sum::<usize>()
                })
                .max()
                .unwrap();
            let admits = |ns| {
                Funding::new(
                    Share::checked(256 << 20, 5, 128 << 20, 512, Arc::new(())).unwrap(),
                    Duration::from_nanos(ns),
                )
                .unwrap()
                .admit(Identity::default(), [0; 6], maximum, Duration::from_nanos(ns))
            };
            let (mut low, mut high) = (1, 1_000_000_000);
            while low < high {
                let mid = low + (high - low) / 2;
                if admits(mid).is_some() {
                    high = mid;
                } else {
                    low = mid + 1;
                }
            }
            assert!(admits(low - 1).is_none());
            let plan = admits(low).unwrap();
            for records in [1u64, 2] {
                let rate = records * 1_000_000_000 / low;
                assert!(rate >= if tdiv < 2e-6 { 1770 } else { 400 });
                println!("F2_MEMORY_BOUNDARY tdiv={tdiv} compress={compress} records={records} fastest_memory_ns={low} B={} max_samples={maximum} capacity_rate={rate} cycle_qualified=false", plan.drain());
            }
        }
    }
}

#[test]
fn configured_cold_remap_and_held_depth_use_final_board_geometry() {
    for tdiv in [1e-6, 10e-6] {
        for compress in [false, true] {
            let _scope = crate::drawpipe::test_scope::Scope::new();
            let sw = crate::sysconf::Switches::parse(
                &|_| None,
                crate::drawpipe::funding::TEST_PROFILE,
                "",
                Default::default(),
            );
            let pipe = crate::drawpipe::start_pipe(&sw);
            let (mut m, geometry, b, p) = normal_lines_plan(tdiv, compress);
            let id = Identity::default();
            for width in [970, 271, 1920, 7680, 970] {
                pipe.publish_view(&m, width, 448, geometry.interp, id);
                assert!(tap::admit(pipe, &b, &p, id));
                let (allocations, accepted) = crate::system::test_allocator::count_own(|| tap::admit(pipe, &b, &p, id));
                assert!(accepted);
                assert_eq!(allocations, 0);
                println!(
                    "F2_CONFIGURED_GEOMETRY tdiv={tdiv} compress={compress} width={width} B={} admitted=true",
                    pipe.tap_drain()
                );
            }
            m.mdepth = crate::model::mdepths().iter().position(|d| d.1 == 1_000_000).unwrap();
            let mut held_sw = sw;
            held_sw.compress = compress;
            let s = super::super::super::settings(&m, false, &held_sw);
            let held = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
            let s = super::super::super::batch_plan(&s, &held, held_sw.board.zone_batch);
            let (allocations, accepted) = crate::system::test_allocator::count_own(|| tap::admit(pipe, &s, &held, id));
            assert!(!accepted, "this staged profile does not fund held-depth raw replay");
            assert_eq!(allocations, 0, "reject before allocating descriptors");
            println!(
                "F2_CONFIGURED_HELD tdiv={tdiv} compress={compress} depth={} rate={} admitted=false",
                held.memsel, held.rate
            );
        }
    }
}

#[test]
fn wide_board_geometry_uses_a_slower_funded_plan_or_the_existing_fallback() {
    use crate::picture::deposit::{count::CountFormat, standard};
    for tap_bytes in [128 << 20, 192 << 20] {
        let _scope = crate::drawpipe::test_scope::Scope::new();
        let conf = format!("f2.tap_bytes={tap_bytes}\n{}", crate::drawpipe::funding::TEST_PROFILE);
        let sw = crate::sysconf::Switches::parse(&|_| None, &conf, "", Default::default());
        let (mut m, _, _, _) = normal_lines_plan(500e-6, false);
        m.mdepth = crate::model::mdepths().iter().position(|d| d.1 == 1_000_000).unwrap();
        m.dots = crate::model::DotsMode::On;
        let s = super::super::super::settings(&m, false, &sw);
        let p = hz::prep(s.mask, oos_hal::dho::seq::tdiv_fs(s.tdiv), s.depth, 0);
        let b = super::super::super::batch_plan(&s, &p, sw.board.zone_batch);
        let pipe = crate::drawpipe::start_pipe(&sw);
        let id = Identity::default();
        pipe.publish_view(&m, 271, 448, sw.interp, id);
        let hits = standard::dot_hit_bound(271.0 / (p.rate as f64 * s.tdiv * 10.0));
        assert_eq!(hits * 24, 69_216);
        assert_eq!(CountFormat::for_hits(hits), CountFormat::Wide);
        let (peak, (admitted, frames)) =
            crate::system::test_allocator::own_high_water(|| deposit_plan(&b, &p, id, true));
        if tap_bytes == 128 << 20 {
            assert!(admitted.is_none());
            assert_eq!(frames, b.frames);
            assert!(peak < p.memsel as usize * std::mem::size_of::<f32>(), "refusal allocated waveform backing");
        } else {
            assert!(readout_pipe(admitted, 1, false, true).is_some());
            assert!(readout_pipe(admitted, 1, false, false).is_none());
            let now = Instant::now();
            assert_eq!(pipe.tap_start(id, now), Some(Duration::ZERO));
            let interval = pipe.tap_start(id, now).unwrap();
            assert!(interval > Duration::from_micros(500));
            assert_eq!(pipe.tap_drain(), 5);
            let fb = oos_hal::dho::seq::frame_bytes(&b, &p);
            let mut bytes = vec![0; fb];
            for word in bytes.as_chunks_mut::<2>().0 {
                word.copy_from_slice(&32768u16.to_le_bytes());
            }
            super::tap_contract::submit_pulse(pipe, (&b, &p), &bytes, (id, 1), 1);
            let mut picture = Phosphor::new(271, 448);
            assert_eq!(pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0)).frames, 1);
            assert!(picture.is_wide());
            assert!(picture.channel_max(0) > u32::from(u16::MAX));
            assert_eq!(pipe.take_waveform((271, 448)).unwrap().seq, 1);
            println!("F2_WIDE hits={hits} B={} interval_ns={}", pipe.tap_drain(), interval.as_nanos());
        }
    }
}
