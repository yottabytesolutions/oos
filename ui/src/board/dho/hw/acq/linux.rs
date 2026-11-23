// SPDX-License-Identifier: GPL-2.0-or-later

use super::super::{overlay, startup, trim};
use super::{
    pacing::DisplayPacing,
    service::{CalRequest, Service},
    status, AcqLoop, Io, Port,
};
use crate::{
    board::{Gen, HwShared, Shared, Spawn},
    sysconf::Switches,
};
use oos_hal::dho::{
    adc, afe,
    bus::{Bar1, Bus},
    seq::{
        self,
        time::{Clock, Real},
    },
};
use std::{
    fs::File,
    sync::{Arc, Mutex},
    time::Duration,
};

pub(super) struct Linux {
    pub(super) afe_bus: Option<afe::GpioAfe>,
}

impl Port for Linux {
    type Bus = Bar1;
    type Stream = File;
    type Clock = Real;

    fn batch_bus<'a>(&self, bus: &'a mut Bar1) -> impl Bus + 'a {
        bus.batch()
    }

    fn apply_settings(io: &mut Io<Self>, steps: &[seq::Step]) -> Result<(), String> {
        seq::execute(steps, &mut io.bar, io.port.afe_bus.as_mut(), &mut io.c2h, |_| {}).map(|_| ())
    }

    fn calibrate(io: &mut Io<Self>, cal_dir: &str, out: &HwShared, req: CalRequest) -> bool {
        super::service::calibrate(io, cal_dir, out, req)
    }

    fn generator(service: &mut Service, bar: &mut Bar1, out: &HwShared, want: Option<Gen>) {
        service.apply_generator(bar, out, want);
    }

    fn probe_la() -> Option<Option<bool>> {
        oos_hal::model::platform().caps.la.then(|| oos_hal::dho::la::probe_present().ok())
    }

    fn trim(io: &mut Io<Self>, mask: u32) -> bool {
        io.cal_adc
            .as_mut()
            .is_some_and(|cal| trim::core_trim(&mut io.bar, &mut io.state, cal, &mut io.trim_overlay, mask))
    }
}

impl Io<Linux> {
    fn open(o: &Spawn, switches: &Switches, out: &HwShared) -> Result<Self, String> {
        let (bar, c2h) = startup::open_acquisition(switches, out)?;
        let afe_bus =
            if o.program_afe { Some(afe::GpioAfe::open().map_err(|e| format!("AFE GPIO: {}", e))?) } else { None };
        let cal = match afe::Cal::load_from(&o.cal_dir) {
            Ok((c, from)) => {
                status(
                    out,
                    format!(
                        "calibration: {}",
                        from.iter().map(|(n, d)| format!("{} from {}", n, d)).collect::<Vec<_>>().join(", ")
                    ),
                );
                Some(c)
            }
            Err(_) => None,
        };
        let mut cal_adc = adc::CalAdc::load(&o.cal_dir).ok();
        // the matched core offsets: only with `cal.core_offset = tight` (key off = the cal_adc.hex above, unchanged)
        let trim_overlay = overlay::in_use(switches.board.core_offset_tight, overlay::load);
        if let Some(c) = cal_adc.as_mut() {
            trim_overlay.apply(c);
        }
        // LA pod zero codes (cal_la.hex, read only); without the file the middle of the DAC range (la.md 8.1)
        let la_cal = if oos_hal::model::platform().caps.la {
            oos_hal::dho::la::Cal::load(&o.cal_dir).unwrap_or_else(|| {
                status(out, "cal_la.hex not found: LA thresholds with the zero code 0x2180".into());
                oos_hal::dho::la::Cal { zero: [0x2180; 2] }
            })
        } else {
            oos_hal::dho::la::Cal::default()
        };
        let state = seq::State::handover(o.mask, cal_adc.as_ref());
        Ok(Self { bar, c2h, port: Linux { afe_bus }, cal, cal_adc, trim_overlay, la_cal, state, clock: Real })
    }
}

fn wait_for_startup(path: Option<&std::path::Path>, out: &HwShared) -> Result<(), String> {
    let Some(path) = path else { return Ok(()) };
    status(out, "waiting for the start-up".into());
    loop {
        let v = oos_hal::bootstatus::parse(&std::fs::read_to_string(path).unwrap_or_default());
        match v.finished {
            Some(Ok(())) => return Ok(()),
            Some(Err(e)) => return Err(format!("acquisition hardware not started: {}", e)),
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

/// Starts the acquisition thread with the hardware left by startup.
pub fn spawn(options: Spawn, switches: Switches) -> HwShared {
    let shared = Arc::new(Mutex::new(Shared::default()));
    let out = shared.clone();
    let spawned = std::thread::Builder::new().name("acq".into()).spawn(move || {
        crate::placement::place_acquisition(&switches);
        let io = wait_for_startup(options.start_up.as_deref().map(std::path::Path::new), &out)
            .and_then(|()| Io::<Linux>::open(&options, &switches, &out));
        match io {
            Ok(mut io) => {
                let now = io.clock.now();
                let service = Service::with_generator_cal(&options.cal_dir);
                let dump = super::super::header::dump::Dump::from_switch(&switches.board.batch_dump);
                AcqLoop::new(options, switches, out, service, dump, now, DisplayPacing).run(&mut io);
            }
            Err(error) => status(&out, error),
        }
    });
    if let Err(e) = spawned {
        eprintln!("oos-ui hw: no acquisition thread: {}", e);
    }
    shared
}
