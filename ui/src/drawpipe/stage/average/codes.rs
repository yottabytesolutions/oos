// SPDX-License-Identifier: GPL-2.0-or-later

use super::{chunk, Batch, Cause, Model, Scratch, NCH};
use crate::fastdraw::Codes;
use crate::picture::deposit::{envelope::Layout, standard::AverageGrid};

pub(super) fn read(
    scratch: &mut Scratch,
    batch: &Batch,
    model: &Model,
    grid: (usize, f64, f64),
    screen_dt: f64,
    index: usize,
) -> Result<(f64, usize, f64, bool), Cause> {
    let read = batch.codes.as_ref().ok_or(Cause::NoAdapter)?;
    let bytes = chunk(batch, index);
    let (channels, len) = fixture_stage!(AverageCodeLayout, {
        let layout = read(index, bytes, &mut scratch.frame).ok_or(Cause::NoWords)?;
        Layout::new(&layout).ok_or(Cause::InvalidLayout)?;
        let len = layout.points(bytes);
        let channels: [Codes<'_>; NCH] = std::array::from_fn(|channel| layout.channel(bytes, channel));
        validate(&channels, len)?;
        Ok::<_, Cause>((channels, len))
    })?;
    let frame = &scratch.frame;
    if frame.envelope {
        return Err(Cause::AveragePairs);
    }
    if !super::valid_geometry(screen_dt, frame.dt, grid.2) {
        return Err(Cause::InvalidGeometry);
    }
    let (ti, triggered) = fixture_stage!(AverageCodeTrigger, {
        crate::board::record::record_trigger_codes(frame, model, grid, &channels[model.trig_src], len)
    })
    .ok_or(Cause::TriggerFromVolts)?;
    if !ti.is_finite() {
        return Err(Cause::InvalidGeometry);
    }
    let grid = AverageGrid::new(grid, frame.dt, ti);
    if !grid.supports_lanczos_edges(len) {
        return Err(Cause::ShortInterpolation);
    }
    for (channel, (codes, out)) in channels.iter().zip(&mut scratch.screen).enumerate() {
        grid.fill(channel, codes, out);
    }
    Ok((frame.dt, frame.pre, ti, triggered))
}

fn validate(channels: &[Codes<'_>; NCH], len: usize) -> Result<(), Cause> {
    for codes in channels.iter().filter(|codes| codes.n != 0) {
        if codes.words.is_none() {
            return Err(Cause::NoWords);
        }
        if codes.n != len {
            return Err(Cause::InvalidLayout);
        }
        if !codes.volts(0).is_finite() || !codes.volts(u16::MAX).is_finite() {
            return Err(Cause::InvalidGeometry);
        }
    }
    Ok(())
}
