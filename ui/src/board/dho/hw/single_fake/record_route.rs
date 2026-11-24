// SPDX-License-Identifier: GPL-2.0-or-later

//! Scripted register and C2H execution on the ordinary record route.

use super::*;
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

#[derive(Default)]
pub(crate) struct RecordWorker {
    state: Option<(Want, seq::State, hz::Params)>,
    done: Option<(u64, u64)>,
    seq: u64,
    dma: DmaBuf,
}

impl RecordWorker {
    pub(crate) fn tick(&mut self, shared: &HwShared, sw: &crate::sysconf::Switches) -> u32 {
        let request = publication::snapshot(shared);
        let want = request.want.unwrap();
        assert_eq!(request.mode, Slow::Off);
        assert!(!request.route && want.segments.is_none());
        let id = request.identity;
        let pair = (id.gen(), id.epoch());
        if single::progress(&want, self.done, pair, request.run.hold, 0).1 != single::Poll::Acquire
            || !lock(shared).queue.is_empty()
            || request.run.segment_bound == Some(0)
        {
            return 0;
        }
        if self.state.as_ref().is_none_or(|(settings, ..)| settings != &want) {
            let mut state = seq::State::handover(want.mask, None);
            let (_, params) = lifecycle::configure(&mut state, &want, None, None).unwrap();
            self.state = Some((want.clone(), state, params));
        }
        let (_, state, params) = self.state.as_mut().unwrap();
        let mut settings = batch_plan(&want, params, sw.board.zone_batch);
        let bound = request.run.batch_bound(settings.frames);
        let grant = publication::record_grant(shared, id, bound, &settings, params, Duration::from_millis(13));
        super::super::dma_source::limit_frames(&mut settings, grant.records(), None, (pair, false, None));
        let count = read(state, &settings, params, &mut self.dma);
        publication::read(crate::drawpipe::counters(), count, count, count as usize);
        // The simulated FPGA serves only current records, so the hardware frame-zero quirk does not apply.
        let (frames, _) = super::super::batch::decode_records(
            (false, id),
            (&settings, params, Default::default()),
            (self.dma.data(), seq::frame_bytes(&settings, params), count as usize),
            (false, false),
            None,
            (&mut self.seq, &mut Default::default()),
        );
        self.done = single::progress(&settings, self.done, pair, false, frames.len() as u32).0;
        publication::push_frames(shared, frames, settings.mode);
        count
    }
}

fn read(state: &mut seq::State, settings: &Settings, params: &hz::Params, dma: &mut DmaBuf) -> u32 {
    let (_, mut bus, mut c2h) = sim::engine(sim::Engine::new(0, 1_000_000_000_000, settings.frames, params.ratio));
    let clock = ScriptClock(Cell::new(Instant::now()));
    lifecycle::batch(settings.single, false, || {
        seq::acquire_batch_with_clock(
            state,
            settings,
            params,
            &mut bus,
            &mut c2h,
            dma,
            false,
            0,
            |_| {},
            || false,
            || false,
            &clock,
        )
    })
    .unwrap()
}

struct ScriptClock(Cell<Instant>);

impl seq::time::Clock for ScriptClock {
    fn now(&self) -> Instant {
        self.0.get()
    }

    fn wait(&self, duration: Duration) {
        self.0.set(self.0.get() + duration);
    }
}
