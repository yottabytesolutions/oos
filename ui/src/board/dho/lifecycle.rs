// SPDX-License-Identifier: GPL-2.0-or-later

//! Count worker hardware operation attempts at the call, independently of publication and settings identity.

use crate::board::counters::Event;
use oos_hal::dho::{adc, afe, bus::Bus, hz, roll, scu::Shadows, seg, seq, Write};

fn count(event: Event) {
    crate::drawpipe::counters().acquisition.add(event);
}

pub(super) fn configure(
    state: &mut seq::State,
    settings: &seq::Settings,
    cal: Option<&afe::Cal>,
    cal_adc: Option<&adc::CalAdc>,
) -> Result<(Vec<seq::Step>, hz::Params), String> {
    count(Event::Configure);
    state.configure(settings, cal, cal_adc)
}

pub(super) fn seg_start(
    state: &mut seq::State,
    settings: &seq::Settings,
    params: &hz::Params,
    bar: &mut impl Bus,
    c2h: &mut impl std::io::Read,
) -> Result<u32, String> {
    count(Event::SegStart);
    seg::start(state, settings, params, bar, c2h)
}

pub(super) fn seg_stop(
    state: &mut seq::State,
    bar: &mut impl Bus,
    c2h: &mut impl std::io::Read,
    abort: bool,
) -> Result<(), String> {
    if abort {
        count(Event::SegAbort);
    }
    seg::stop(state, bar, c2h)
}

pub(super) fn roll_start(shadows: &mut Shadows, mask: u32) -> Vec<Write> {
    count(Event::RollStart);
    roll::start(shadows, mask)
}

pub(super) fn roll_stop(shadows: &mut Shadows) -> Vec<Write> {
    count(Event::RollStop);
    roll::stop(shadows)
}

/// The closures retain the unit's existing executor; host fakes execute the same arm/disarm steps.
pub(super) fn prearm<T>(single: bool, arm: impl FnOnce() -> T) -> T {
    count(Event::BatchPrearm);
    if single {
        count(Event::SingleArm);
    }
    arm()
}

pub(super) fn disarm<T>(stop: impl FnOnce() -> T) -> T {
    count(Event::BatchDisarm);
    stop()
}

pub(super) fn batch<T>(single: bool, ahead: bool, acquire: impl FnOnce() -> T) -> T {
    if single && !ahead {
        count(Event::SingleArm);
    }
    acquire()
}

#[cfg(test)]
mod tests;
