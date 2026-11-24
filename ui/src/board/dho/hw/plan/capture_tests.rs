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
