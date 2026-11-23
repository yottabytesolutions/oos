// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::drawpipe::budget::WORK_BUDGET;
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
