// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::drawpipe::budget::WORK_BUDGET;
mod played_cycle;
mod two_channel;
use crate::model::{Model, RunState};
fn fixture() -> (Pipe, Settings, hz::Params, Identity, Model) {
    let pipe = Pipe::start(1, 0, Box::leak(Box::default()));
    let id = Identity::default();
    let mut model = Model::default();
    for (key, value) in [
        ("tdiv", "0.000001"),
        ("trig_mode", "Normal"),
        ("trig_type", "Edge"),
        ("acq", "Normal"),
        ("mdepth", "0"),
        ("cores", "true"),
        ("dots", "Auto"),
        ("persist", "0"),
        ("meas", ""),
        ("meas2", ""),
        ("ch1.on", "true"),
        ("ch2.on", "true"),
        ("ch3.on", "true"),
        ("ch4.on", "true"),
    ] {
        crate::store::set(&mut model, key, value).unwrap();
    }
    model.run = RunState::Run;
    let switches = crate::sysconf::Switches::default();
    let original = crate::board::dho::hw::settings(&model, false, &switches);
    let params = hz::prep(original.mask, oos_hal::dho::seq::tdiv_fs(original.tdiv), original.depth, 0);
    let mut settings = crate::board::dho::hw::batch_plan(&original, &params, switches.board.zone_batch);
    settings.header = false;
    assert_eq!(oos_hal::dho::seq::frame_bytes(&settings, &params), 28160);
    pipe.publish_view(&model, 970, 448, crate::sparse::Interp::Exact, id);
    (pipe, settings, params, id, model)
}
#[test]
fn cold_read_grant_respects_the_hardware_and_plane_bounds() {
    let (pipe, settings, _, id, _) = fixture();
    assert_eq!(read_grant(Some(&pipe), id, settings.frames, WORK_BUDGET).records(), 12);
    assert_eq!(read_grant(Some(&pipe), id, 7, WORK_BUDGET).records(), 7);
    assert_eq!(read_grant(None, id, settings.frames, WORK_BUDGET).records(), settings.frames);
}

#[test]
fn default_proposal_reaches_actual_readout_worker_without_truncating_raw_capture() {
    use crate::board::scripted::{self, Batch as _};
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let (pipe, settings, params, id, model) = fixture();
    let count = read_grant(Some(&pipe), id, 142, WORK_BUDGET).records();
    assert_eq!(count, 12);
    let (input, actual, _) = scripted::draw_route_batch(&model, &Default::default(), count as usize);
    assert_eq!(actual, count as usize);
    let fb = input.frame_bytes();
    let mut raw = Some(input.into_bytes());
    let frames = Readout::new(
        &pipe,
        (&settings, &params),
        (fb, actual, false),
        (actual as u64, crate::board::dho::switches::HeaderProbe::Off, None),
        (id, Instant::now()),
    )
    .dispatch(&mut raw, Some(&mut Cores::default()), |f| f, &|| false);
    assert!(frames.is_empty());
    assert!(raw.is_none());
    let recycled = crate::drawpipe::host::finish_batch(&pipe, 0).unwrap();
    assert_eq!(recycled.bytes().len(), actual * fb);
    let mut picture = crate::wave::Phosphor::new(970, 448);
    let accepted = pipe.accept(&mut picture, &mut true, &mut [0; 4], &mut (0.0, 0.0));
    assert_eq!(accepted.frames, u64::from(count));
    assert!(picture.maps().iter().all(|ch| ch.iter().any(|&px| px > 0)));
}

#[test]
fn dense_average_plans_from_worker_cost_and_raw_dma_capacity() {
    for (lanes, tdiv, per_record_us) in
        [(1, "0.000001", 172), (1, "0.000010", 700), (4, "0.000001", 705), (4, "0.000010", 900)]
    {
        check_dense_average_grant(lanes, tdiv, Duration::from_micros(per_record_us));
    }
}

fn check_dense_average_grant(lanes: usize, tdiv: &str, per_record: Duration) {
    use crate::board::{
        dho::hw,
        scripted::{self, Batch as _},
    };
    let _scope = crate::drawpipe::test_scope::Scope::new();
    let mut model = Model { acq: crate::model::AcqMode::Average, avg_count: 16, ..Default::default() };
    crate::store::set(&mut model, "tdiv", tdiv).unwrap();
    for (i, ch) in model.ch.iter_mut().enumerate() {
        ch.on = i < lanes;
    }
    let switches = crate::sysconf::Switches {
        compress: true,
        cores: crate::placement::Cores { big: 0, little: 0b1111 },
        ..Default::default()
    };
    let pipe = crate::drawpipe::start_pipe(&switches);
    let id = Identity::default();
    pipe.publish_view(&model, 970, 448, switches.interp, id);
    let settings = hw::settings(&model, false, &switches);
    let params = hz::prep(settings.mask, oos_hal::dho::seq::tdiv_fs(settings.tdiv), settings.depth, 0);
    let mut b = hw::batch_plan(&settings, &params, switches.board.zone_batch);
    b.header = false;
    let (deposit, bound) = deposit_plan(&b, &params, id, true);
    assert!(deposit.is_some());
    assert_eq!(bound, b.frames, "the Average transform does not occupy difference planes");
    assert_eq!(super::super::super::decode::pair_step(&b, &params), 1);
    let cold = read_grant(deposit, id, bound, WORK_BUDGET);
    assert!(!cold.serial() && cold.records() >= 4);
    let (batch, records, _) = scripted::draw_route_batch(&model, &switches, 4);
    let fb = batch.frame_bytes();
    assert_eq!(fb, oos_hal::dho::seq::frame_bytes(&b, &params));
    let at = StageClock::from(Instant::now()).with_admission().pool_for_test(per_record);
    let mut raw = Some(batch.into_bytes());
    let counters = pipe.batch_counters();
    crate::drawpipe::Counters::add(&counters.acquired, records as u64);
    let unread = Readout::new(pipe, (&b, &params), (fb, records, false), (4, HeaderProbe::Off, None), (id, at))
        .dispatch(&mut raw, None, |f| f, &|| false);
    assert!(unread.is_empty() && raw.is_none());
    let returned = crate::drawpipe::host::finish_batch(pipe, 0).unwrap();
    assert_eq!(returned.bytes().len(), records * fb);
    assert_eq!(pipe.take_waveform((970, 448)).unwrap().read(&model).1.unwrap().shown().0, records);
    let counts = counters.records_text();
    assert!(counts.starts_with(" records read=4 deposited=4 "), "{counts}");
    assert_eq!(counters.read().2, 0);

    let warm = read_grant(deposit, id, bound, WORK_BUDGET);
    let expected = ((4 * WORK_BUDGET.as_nanos() / per_record.as_nanos()) as u32).min(bound);
    assert_eq!(warm, Grant::Overlapping(expected));
    println!("lanes={lanes} tdiv={tdiv} frame_bytes={fb} bound={bound} cold={} warm={expected}", cold.records());
}
