// SPDX-License-Identifier: GPL-2.0-or-later

//! Worker view publication and batch eligibility.

use super::*;

pub(crate) fn same_pair(a: Option<oos_core::change::Identity>, b: Option<oos_core::change::Identity>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (a.gen(), a.epoch()) == (b.gen(), b.epoch()),
        (None, None) => true,
        _ => false,
    }
}

pub(in crate::drawpipe) fn batch_view(b: &Batch, state: &ViewState, reset_before: u64) -> Option<Arc<View>> {
    let v = state
        .view
        .clone()
        .filter(|_| b.view_gen_at_submit >= reset_before && accepts(b, legacy_stamp(state), state.identity));
    if v.is_none() {
        let records = b.frames.saturating_sub(b.next.swap(b.frames, Ordering::Relaxed)) as u64;
        if !same_pair(b.header.identity(), state.identity) {
            Counters::add(&b.counters.losses.pipe, records);
        }
        b.counters.discard(Lost::Settings, records);
    }
    v
}

fn accepts(b: &Batch, legacy_stamp: u64, identity: Option<oos_core::change::Identity>) -> bool {
    match (b.header.identity(), identity) {
        (Some(_), Some(_)) => same_pair(b.header.identity(), identity),
        #[cfg(test)]
        (None, None) => b.header.stamp() >= legacy_stamp,
        _ => {
            let _ = legacy_stamp;
            false
        }
    }
}

fn legacy_stamp(state: &ViewState) -> u64 {
    #[cfg(test)]
    {
        state.stamp
    }
    #[cfg(not(test))]
    {
        let _ = state;
        0
    }
}

pub(in crate::drawpipe) fn set_view(
    pipe: &Pipe,
    m: Option<&Model>,
    (w, h): (usize, usize),
    interp: Interp,
    identity: Option<oos_core::change::Identity>,
    zoom: Option<route::ZoomView>,
) {
    let mut v = oos_hal::sync::lock(&pipe.view);
    let Some(m) = m else {
        (v.view, v.key) = (None, None);
        pipe.retire_tap_plan();
        return;
    };
    let layout = zoom
        .map_or_else(|| oos_core::change::Layout::new((w, h), (0, 0), (0, 0), None, 0, 0), |zoom| zoom.layout((w, h)));
    let key = oos_core::change::WorkerViewKey::of(m, layout, &crate::model::config());
    let deskew = zoom.is_some() && v.view.as_ref().is_some_and(|old| old.m.deskew_ch != m.deskew_ch);
    let worker_changed = v.key.as_ref() != Some(&key) || deskew;
    let statistics = identity.is_some_and(|id| v.identity.map(|old| old.stats_gen()) != Some(id.stats_gen()));
    if worker_changed {
        let mapping_key = oos_core::change::MappingKey::of(m, layout);
        v.mapping_gen += u64::from(v.mapping_key.as_ref() != Some(&mapping_key) || deskew);
        v.mapping_key = Some(mapping_key);
    }
    if worker_changed || statistics {
        crate::pacing::VIEWS.fetch_add(1, Ordering::Relaxed);
        v.gen += u64::from(worker_changed);
        v.view = Some(Arc::new(View { gen: v.gen, m: m.clone(), interp, w, h, zoom }));
        v.key = Some(key);
    }
    #[cfg(test)]
    {
        v.stamp = m.hw_stamp;
    }
    if let Some(id) = identity {
        if !same_pair(v.identity, Some(id)) {
            pipe.counters.metrics.set_identity(id);
            pipe.admission.set_identity(id);
            pipe.source.notify();
            pipe.slot.set_tap_identity(id);
        }
        v.identity = Some(id);
    }
}
