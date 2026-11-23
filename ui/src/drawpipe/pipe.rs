// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    admission, counters, feed, funding, route, shutdown, Counters, Message, PreparedView, RawBatch, SourceWake,
};
#[cfg(test)]
use super::{stage, test_scope, Accepted, Stats};
use crate::board::{Frame, PictureEligibility, QueuedFrame};
use crate::model::Model;
use crate::picture::deposit::{Handoff, TapProjection, TapRead, TapView};
#[cfg(test)]
use crate::{picture::deposit::Deposit, wave::Phosphor};
use crate::{picture::DepositWindow, sparse::Interp};
use oos_core::change::{Identity, MappingKey, WorkerViewKey};
use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};

pub(super) struct View {
    pub(super) gen: u64,
    pub(super) m: Model,
    pub(super) interp: Interp,
    pub(super) w: usize,
    pub(super) h: usize,
    pub(super) zoom: Option<route::ZoomView>,
}

#[derive(Clone, Default)]
pub(super) struct ViewState {
    pub(super) view: Option<Arc<View>>,
    pub(super) spare: Option<Arc<View>>,
    #[cfg(test)]
    pub(super) stamp: u64,
    pub(super) identity: Option<Identity>,
    pub(super) key: Option<WorkerViewKey>,
    pub(super) gen: u64,
    pub(super) mapping_key: Option<MappingKey>,
    pub(super) mapping_gen: u64,
}

impl ViewState {
    pub(super) fn tap_view(&self, size: (usize, usize)) -> Option<TapView> {
        self.view.as_ref().map(|v| TapView::new(v.gen, self.mapping_gen, size))
    }
}

pub struct Pipe {
    pub(super) view: Arc<Mutex<ViewState>>,
    pub(super) counters: &'static Counters,
    pub(super) workers: Mutex<shutdown::Workers>,
    pub(super) admission: Arc<admission::Admission>,
    pub(super) slot: Arc<Handoff>,
    pub(super) live: Arc<AtomicUsize>,
    pub(super) home: Sender<Box<dyn RawBatch>>,
    pub(super) source: Arc<SourceWake>,
    pub(super) pool: Mutex<Receiver<Box<dyn RawBatch>>>,
}

impl Pipe {
    #[cfg(target_os = "linux")]
    pub(crate) fn fixture_view_generation(&self) -> u64 {
        oos_hal::sync::lock(&self.view).gen
    }

    pub fn set_view(&self, m: Option<&Model>, w: usize, h: usize, interp: Interp) {
        feed::set_view(self, m, (w, h), interp, None);
    }

    #[cfg(test)]
    pub fn publish_view(&self, m: &Model, w: usize, h: usize, interp: Interp, identity: Identity) {
        self.publish_picture_view(m, (w, h), interp, identity, None);
    }

    #[cfg(test)]
    pub(crate) fn publish_picture_view(
        &self,
        m: &Model,
        size: (usize, usize),
        interp: Interp,
        identity: Identity,
        zoom: Option<route::ZoomView>,
    ) {
        if feed::set_view(self, Some(m), size, interp, Some((identity, zoom))) {
            self.source.notify();
            self.slot.wake_tap_commands();
        }
    }

    /// Only the UI publisher writes view state. Workers can keep reading the old view during preparation.
    pub(crate) fn prepare_picture_view(
        &self,
        m: &Model,
        size: (usize, usize),
        interp: Interp,
        identity: Identity,
        zoom: Option<route::ZoomView>,
    ) -> PreparedView {
        feed::prepare_view(self, m, size, interp, identity, zoom)
    }

    /// Install the prepared view and stage tap invalidation before the Shared publisher wakes the source.
    pub(crate) fn stage_picture_view(&self, prepared: &mut PreparedView, active: bool, identity: Identity) {
        feed::stage_view(self, prepared, active, identity);
    }

    pub fn set_identity(&self, identity: Identity) {
        let mut view = oos_hal::sync::lock(&self.view);
        if feed::stage_identity(self, &mut view, identity, true) {
            self.source.notify();
            self.slot.wake_tap_commands();
        }
    }

    pub fn accepting(&self) -> bool {
        !oos_hal::sync::lock(&self.workers).closed && oos_hal::sync::lock(&self.view).view.is_some()
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn source_generation(&self) -> u64 {
        self.source.generation()
    }

    #[cfg(test)]
    pub(crate) fn command_generations(&self) -> (u64, u64) {
        (self.source_generation(), self.slot.tap_command_generation())
    }

    /// Return on DMA release or a new command. A deadline keeps the acquisition thread's
    /// periodic instrument housekeeping running while the FPGA is unarmed.
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn wait_source(&self, observed: u64, deadline: std::time::Duration) -> Option<std::time::Duration> {
        self.source.wait(observed, deadline)
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn note_source_wait(&self, identity: Identity, elapsed: Option<std::time::Duration>) {
        if let Some(elapsed) = elapsed {
            self.counters.metrics.source_wait(identity, elapsed);
        }
    }
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn return_buffer(&self, buffer: Box<dyn RawBatch>) {
        if self.home.send(buffer).is_ok() {
            self.source.notify();
        }
    }

    pub fn take_buffer(&self) -> Option<Box<dyn Any + Send>> {
        oos_hal::sync::lock(&self.pool).try_recv().ok().map(|r| r.into_any())
    }

    #[cfg(test)]
    pub fn accept(&self, ph: &mut Phosphor, clear: &mut bool, edges: &mut [u64; 4], off: &mut (f64, f64)) -> Accepted {
        self.accept_into(ph, clear, Stats { edges, off }, None)
    }

    pub(crate) fn discard_window(&self, window: &mut DepositWindow) {
        window.reset(&self.slot);
    }

    #[cfg(test)]
    pub(crate) fn take_recycled_deposit(&self) -> Option<Deposit> {
        self.slot.spare()
    }

    pub(crate) fn tap_count(&self) -> usize {
        self.slot.tap_count()
    }

    pub(crate) fn has_waveform(&self, identity: Identity) -> bool {
        self.slot.has_tap(identity)
    }

    #[cfg(test)]
    pub(crate) fn take_waveform(&self, size: (usize, usize)) -> Option<QueuedFrame> {
        let identity = oos_hal::sync::lock(&self.view).identity;
        match self.read_waveform(size, identity, None, true) {
            TapRead::Ready((frame, _)) => Some(frame),
            _ => None,
        }
    }

    pub(crate) fn read_waveform(
        &self,
        size: (usize, usize),
        identity: Option<Identity>,
        fallback: Option<u64>,
        can_take: bool,
    ) -> TapRead<(QueuedFrame, Option<crate::framestats::TapFolds>)> {
        let state = oos_hal::sync::lock(&self.view);
        let view = state.tap_view(size);
        let (tap, charge) = match self.slot.take_tap(view, identity, fallback, can_take) {
            TapRead::Ready(tap) => tap,
            TapRead::Stale => return TapRead::Stale,
            TapRead::Pending => return TapRead::Pending,
            TapRead::Empty => return TapRead::Empty,
        };
        let eligibility = tap.projection.eligibility(view);
        let average = match tap.projection {
            TapProjection::Average(output) => Some(output),
            _ => None,
        };
        TapRead::Ready((QueuedFrame::from_tap(tap.frame, eligibility, charge, average), tap.folds))
    }

    pub(crate) fn take_preview(&self, frame: &Arc<Frame>, size: (usize, usize)) -> Option<PictureEligibility> {
        let state = oos_hal::sync::lock(&self.view);
        let id = state.identity?;
        if (id.gen(), id.epoch()) != (frame.cfg, frame.stamp) {
            return None;
        }
        self.slot.take_preview(state.tap_view(size), frame)
    }

    #[cfg(test)]
    pub(crate) fn accept_into(
        &self,
        ph: &mut Phosphor,
        clear: &mut bool,
        stats: Stats<'_>,
        window: Option<&mut DepositWindow>,
    ) -> Accepted {
        stage::owner::Owner::default().compose_owned(self, ph, clear, stats, window)
    }

    pub(super) fn wake(&self) {
        let workers = oos_hal::sync::lock(&self.workers);
        let Some(tx) = &workers.sender else { return };
        if !workers.wake_pending.swap(true, Ordering::AcqRel) {
            // A full channel already makes the coordinator runnable. Keep the request pending either way.
            let _ = tx.try_send(Message::Wake);
        }
    }
}

fn pipe_slot() -> &'static OnceLock<Pipe> {
    #[cfg(test)]
    if let Some(state) = test_scope::state() {
        return &state.pipe;
    }
    static P: OnceLock<Pipe> = OnceLock::new();
    &P
}

/// Return the current batch pipeline without starting workers.
pub fn pipe() -> Option<&'static Pipe> {
    pipe_slot().get()
}

/// Start the batch pipeline on first use.
pub fn start_pipe(sw: &crate::sysconf::Switches) -> &'static Pipe {
    pipe_slot().get_or_init(|| {
        #[cfg(test)]
        if let Some(state) = test_scope::state() {
            return funding::application(sw, state.counters);
        }
        funding::application(sw, counters())
    })
}
