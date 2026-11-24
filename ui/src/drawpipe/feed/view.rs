// SPDX-License-Identifier: GPL-2.0-or-later

//! Worker view publication and batch eligibility.

use super::*;

#[derive(Default)]
pub(crate) struct PreparedView {
    state: ViewState,
    changed: bool,
    retired: Option<Arc<View>>,
}

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
    publication: Option<(Identity, Option<route::ZoomView>)>,
) -> bool {
    let mut v = oos_hal::sync::lock(&pipe.view);
    let identity = publication.map(|(id, _)| id);
    if let Some(m) = m {
        if update_view(&mut v, m, (w, h), interp, identity, publication.and_then(|(_, zoom)| zoom)) {
            crate::pacing::VIEWS.fetch_add(1, Ordering::Relaxed);
        }
    } else {
        (v.view, v.key) = (None, None);
    }
    if let Some(id) = identity {
        return stage_identity(pipe, &mut v, id, m.is_some());
    }
    if m.is_none() {
        pipe.retire_tap_plan();
    }
    false
}

pub(in crate::drawpipe) fn prepare_view(
    pipe: &Pipe,
    m: &Model,
    size: (usize, usize),
    interp: Interp,
    identity: Identity,
    zoom: Option<route::ZoomView>,
) -> PreparedView {
    let mut current = oos_hal::sync::lock(&pipe.view);
    let spare = current.spare.take();
    let mut state = current.clone();
    drop(current);
    state.spare = spare;
    let changed = update_view(&mut state, m, size, interp, Some(identity), zoom);
    PreparedView { state, changed, retired: None }
}

pub(in crate::drawpipe) fn stage_view(pipe: &Pipe, prepared: &mut PreparedView, active: bool, identity: Identity) {
    let mut view = oos_hal::sync::lock(&pipe.view);
    // Leave the displaced view with the caller so its model can be dropped after Shared is released.
    if active {
        std::mem::swap(&mut *view, &mut prepared.state);
        if prepared.changed {
            std::mem::swap(&mut view.spare, &mut prepared.state.view);
            crate::pacing::VIEWS.fetch_add(1, Ordering::Relaxed);
        }
    } else {
        std::mem::swap(&mut view.view, &mut prepared.retired);
        view.key = None;
    }
    stage_identity(pipe, &mut view, identity, active);
}

pub(in crate::drawpipe) fn stage_identity(pipe: &Pipe, v: &mut ViewState, id: Identity, active: bool) -> bool {
    let changed = !same_pair(v.identity, Some(id));
    if changed {
        pipe.counters.metrics.set_identity(id);
        pipe.admission.set_identity(id);
    }
    pipe.slot.stage_tap_view(id, active);
    v.identity = Some(id);
    changed
}

fn update_view(
    v: &mut ViewState,
    m: &Model,
    (w, h): (usize, usize),
    interp: Interp,
    identity: Option<Identity>,
    zoom: Option<route::ZoomView>,
) -> bool {
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
        v.gen += u64::from(worker_changed);
        let mut spare = v.spare.take();
        // Readers may still hold the retired projection. Reuse its storage only when the publisher owns it alone.
        v.view = if let Some(view) = spare.as_mut().and_then(Arc::get_mut) {
            oos_core::change::refresh(&mut view.m, m);
            (view.gen, view.interp, view.w, view.h, view.zoom) = (v.gen, interp, w, h, zoom);
            spare
        } else {
            Some(Arc::new(View { gen: v.gen, m: m.clone(), interp, w, h, zoom }))
        };
        v.key = Some(key);
    }
    #[cfg(test)]
    {
        v.stamp = m.hw_stamp;
    }
    worker_changed || statistics
}
