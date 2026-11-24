// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::sysconf::Switches;
use oos_hal::dho::{acq::Mode, seq};

fn at_1us() -> Model {
    Model {
        tscale: tscales().iter().position(|&t| (t - 1e-6).abs() < 1e-12).unwrap(),
        delay: 0.0,
        ..Default::default()
    }
}

fn switches(conf: &str) -> Switches {
    Switches::parse(&|_| None, conf, "", Default::default())
}

#[test]
fn capture_switch_programs_and_reads_the_same_geometry() {
    let m = at_1us();
    let full = settings(&m, false, &Switches::default());
    assert_eq!(settings(&m, false, &switches("acq.capture = full")), full);
    assert_eq!(settings(&m, false, &switches("acq.capture = peak:4")), full);
    assert_eq!(settings(&m, false, &switches("acq.capture = decimate:64\nacq.capture.inferred = on")), full);

    let sw = switches("acq.capture = peak:4\nacq.capture.inferred = on\nacq.compress = on");
    let s = settings(&m, false, &sw);
    assert!(matches!(s.mode, Mode::Reduced(capture) if capture.to_string() == "peak:4"));
    let (_, p) = seq::State::handover(s.mask, None).configure(&s, None, None).unwrap();
    assert_eq!((p.rate, p.memsel), (312_500_000, 3512));
    let b = batch_plan(&s, &p, 1);
    assert_eq!((b.compress, b.window, seq::frame_bytes(&b, &p)), (1, None, 7024));
    assert!(b.frames <= super::super::super::PEAK_BATCH_MAX as u32);
}

#[test]
fn play_window_switch_never_crops_reduced_capture_or_its_reference() {
    use super::super::window::{batch_play, configured_play_window};
    let m = at_1us();
    for capture in ["peak:4", "decimate:2"] {
        let sw = switches(&format!(
            "acq.capture = {capture}\nacq.capture.inferred = on\nacq.compress = on\nacq.play_window = on"
        ));
        let s = settings(&m, false, &sw);
        assert!(sw.source.play_window());
        assert!(matches!(s.mode, Mode::Reduced(_)));
        let p = seq::record_geometry(&s).unwrap();
        let play = configured_play_window(&s, &p, 1024);
        assert_eq!(play, None);
        let b = batch_play(&s, &p, 1, play);
        assert_eq!((b.window, b.compress, seq::window(&b, &p)), (None, 1, (p.memsel, 0)));
        let reference = super::super::raw::reference_settings(&s);
        let full = seq::record_geometry(&reference).unwrap();
        let b = batch_plan(&reference, &full, 1);
        assert_eq!((b.window, b.compress, seq::window(&b, &full)), (None, 1, (full.memsel, 0)));
    }
}

#[test]
fn diagnostic_decimation_programs_a_reduced_record_with_dots_enabled() {
    let model = Model { dots: DotsMode::On, ..at_1us() };
    let sw = switches("acq.capture = decimate:2\nacq.capture.inferred = on");
    let decimated = settings(&model, false, &sw);
    assert!(matches!(decimated.mode, Mode::Reduced(capture) if capture.to_string() == "decimate:2"));
    let (_, p) = seq::State::handover(decimated.mask, None).configure(&decimated, None, None).unwrap();
    assert_eq!((p.rate, p.memsel), (625_000_000, 7032));
}

#[test]
fn capture_switch_leaves_explicit_depth_and_special_acquisition_alone() {
    let sw = switches("acq.capture = decimate:2\nacq.capture.inferred = on");
    for m in [
        Model { mdepth: 1, ..at_1us() },
        Model { acq: AcqMode::Peak, ..at_1us() },
        Model { delay: 1e-9, ..at_1us() },
        Model { segments: 4, ..at_1us() },
        Model { mask_on: true, ..at_1us() },
    ] {
        assert_eq!(settings(&m, false, &sw), settings(&m, false, &Switches::default()));
    }
}

#[test]
fn capture_state_reports_geometry_only_for_current_frames() {
    let board = crate::board::board();
    let shared = crate::board::HwShared::default();
    let source = crate::wave::Source::Hw { shared: shared.clone(), seen: 0 };
    let state = || board.capture_state(&source);
    assert_eq!(state(), " acq.capture=pending acq.capture.stored_points=0");

    let model = at_1us();
    let reduced = settings(&model, false, &switches("acq.capture = peak:4\nacq.capture.inferred = on"));
    let mut g = crate::perfguard::deep::acq_lock(&shared);
    let id = g.acquisition.identity();
    g.want = Some(reduced);
    g.frame = std::sync::Arc::new(crate::board::Frame {
        seq: 1,
        cfg: id.gen(),
        stamp: id.epoch(),
        acq: (312_500_000.0, 3512),
        ..Default::default()
    });
    drop(g);
    assert_eq!(state(), " acq.capture=peak:4 acq.capture.stored_points=3512");
}
