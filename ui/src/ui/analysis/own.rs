// SPDX-License-Identifier: GPL-2.0-or-later
//! Per-row parameters share the normal physical gate and its one bounded worker.
//! Scalar results remain cached after large edge analyses are evicted.

use super::super::reflook::MeasOwn;
use super::*;
use std::collections::VecDeque;

const ANALYSIS_LIMIT: usize = 4;
const SCALAR_LIMIT: usize = super::super::tools::MEAS_MAX * (NREF + 1);

#[derive(Clone, Copy, PartialEq, Eq)]
struct Key {
    source: WaveId,
    analysis: RefMeasureKey,
}

#[derive(Clone, Copy)]
struct Request {
    key: Key,
    kind: Meas,
    owner: WaveId,
}

struct Cached {
    key: Key,
    analysis: Option<Arc<oos_dsp::measure::Analysis>>,
    readings: Vec<f64>,
}

#[derive(Default)]
pub(super) struct State {
    cached: VecDeque<Cached>,
    pending: VecDeque<Request>,
}

fn analysis_key(mut key: RefMeasureKey) -> RefMeasureKey {
    key.idle = 0;
    key.outside = 0;
    key.periods = 0;
    key.data = false;
    key.basic = false;
    key
}

/// The longest a measurement query waits for the background measurement of the record shown.
const QUERY_WAIT: std::time::Duration = std::time::Duration::from_secs(3);

impl Ui {
    /// The channels `measure_rec` measures on this thread: on, with at most `limit` samples to measure (`meas_slice`).
    /// A larger raw window is measured by the background worker (`measure_channel_sources`).
    pub(in crate::ui) fn measure_work<'a>(&self, rec: &'a Record, limit: usize) -> Vec<(usize, &'a [f32], f64, f64)> {
        (0..NCH)
            .filter(|&c| self.m.ch[c].on)
            .map(|c| {
                let (v, dt) = self.meas_slice(rec, c);
                (c, v, dt, 0.4 * self.m.ch[c].volts_per_div() as f64)
            })
            .filter(|w| w.1.len() <= limit)
            .collect()
    }

    /// For a remote measurement query: the background measurements of the record shown, started if they are not yet,
    /// waited for up to `QUERY_WAIT`. A job that does not finish in time leaves its value unavailable; a result for other
    /// settings or another record is never taken (the keys of `channel_measurement`).
    pub(crate) fn measurement_settle(&mut self) {
        let time = &self.host.runtime().query_time;
        let (now, wait) = (time.now.clone(), time.wait.clone());
        self.measurement_settle_with(|ui| now(ui), |ui| wait(ui));
    }

    fn measurement_settle_with(&mut self, now: impl Fn(&Self) -> std::time::Instant, wait: impl Fn(&Self)) {
        let deadline = now(self) + QUERY_WAIT;
        self.poll_reference_measurements();
        self.measure_channel_sources();
        while self.measurement_busy() && now(self) < deadline {
            wait(self);
            self.poll_reference_measurements();
        }
        self.measure_channel_sources();
    }

    fn own_params(&self, kind: Meas, owner: WaveId) -> Option<MeasOwn> {
        let own = self.meas_own.get(&(kind, owner)).copied()?;
        if !(own.refs_set
            || (kind == Meas::Burst && own.idle_set)
            || (kind == Meas::Outside && own.side_set)
            || (kind == Meas::NPeriods && (own.n_set || own.data_set)))
        {
            return None;
        }
        let shared = MeasOwn::shared(&self.m);
        Some(MeasOwn {
            low: if own.refs_set { own.low } else { shared.low },
            high: if own.refs_set { own.high } else { shared.high },
            idle: if own.idle_set { own.idle } else { shared.idle },
            side: if own.side_set { own.side } else { shared.side },
            n: if own.n_set { own.n } else { shared.n },
            data: if own.data_set { own.data } else { shared.data },
            ..own
        })
    }

    fn own_key(&self, source: WaveId, own: MeasOwn) -> Option<(Key, f64)> {
        let (mut key, t0) = match source {
            WaveId::Ch(c) => self.channel_measurement_key(c, false)?,
            WaveId::Ref(i) => self.reference_measurement_key(i)?,
        };
        key.low = own.low.to_bits();
        key.high = own.high.to_bits();
        key.idle = own.idle.to_bits();
        key.outside = own.side as u8;
        key.periods = own.n;
        key.data = own.data;
        key.basic = false;
        Some((Key { source, analysis: key }, t0))
    }

    fn request_current(&self, request: Request) -> bool {
        self.own_params(request.kind, request.owner)
            .and_then(|own| self.own_key(request.key.source, own))
            .is_some_and(|(key, _)| key == request.key)
    }

    fn reusable_own_analysis(&self, key: Key) -> Option<Arc<oos_dsp::measure::Analysis>> {
        let expected = analysis_key(key.analysis);
        let cache = match key.source {
            WaveId::Ch(c) => self.azr.ch_measure.get(c)?,
            WaveId::Ref(i) => self.azr.ref_measure.get(i)?,
        }
        .borrow();
        if let Some(cache) = cache.as_ref().filter(|cache| !cache.key.basic && analysis_key(cache.key) == expected) {
            return Some(Arc::clone(&cache.analysis));
        }
        self.azr
            .own_measure
            .borrow()
            .cached
            .iter()
            .rev()
            .find(|cache| {
                cache.key.source == key.source
                    && analysis_key(cache.key.analysis) == expected
                    && cache.analysis.is_some()
            })
            .and_then(|cache| cache.analysis.as_ref().map(Arc::clone))
    }

    fn own_job(&self, key: Key) -> Option<RefMeasureJob> {
        let analysis = self.reusable_own_analysis(key);
        let k = key.analysis;
        let samples = if analysis.is_some() {
            MeasureSamples::Owned(Vec::new())
        } else {
            match key.source {
                WaveId::Ref(i) => MeasureSamples::Owned(self.refs.get(i)?.as_ref()?.v[k.lo..k.hi].to_vec()),
                WaveId::Ch(c) => {
                    let record = self.last[0].as_ref()?;
                    let (values, _, _) = self.measurement_channel_samples(record, c)?;
                    if let Some(raw) = record.raw.as_ref().filter(|raw| {
                        let original = &raw.frame.ch[c];
                        let start = values.as_ptr() as usize;
                        start >= original.as_ptr() as usize
                            && start + std::mem::size_of_val(values)
                                <= original.as_ptr() as usize + std::mem::size_of_val(original.as_slice())
                    }) {
                        let lo =
                            (values.as_ptr() as usize - raw.frame.ch[c].as_ptr() as usize) / std::mem::size_of::<f32>();
                        MeasureSamples::Native { frame: Arc::clone(&raw.frame), channel: c, lo, hi: lo + values.len() }
                    } else {
                        MeasureSamples::Owned(values.to_vec())
                    }
                }
            }
        };
        Some(RefMeasureJob {
            source: key.source,
            key: k,
            samples,
            dt: f64::from_bits(k.dt),
            min_amp: 0.4 * f32::from_bits(k.vpd) as f64,
            low: f64::from_bits(k.low),
            high: f64::from_bits(k.high),
            own: true,
            analysis,
        })
    }

    fn cache_own_result(&self, key: Key, analysis: Arc<oos_dsp::measure::Analysis>, readings: Vec<f64>) {
        let mut state = self.azr.own_measure.borrow_mut();
        state.cached.retain(|cached| cached.key != key);
        state.cached.push_back(Cached { key, analysis: Some(analysis), readings });
        while state.cached.len() > SCALAR_LIMIT {
            state.cached.pop_front();
        }
        // A scalar-only entry still answers all row reads; do not rescan because
        // its large edge arrays no longer occupy one of the four retained slots.
        let mut retained = 0;
        for cached in state.cached.iter_mut().rev() {
            if cached.analysis.is_some() {
                retained += 1;
                if retained > ANALYSIS_LIMIT {
                    cached.analysis = None;
                }
            }
        }
    }

    pub(in crate::ui) fn measurement_with_own(&self, source: WaveId, kind: Meas, owner: WaveId) -> f64 {
        let Some(own) = self.own_params(kind, owner) else { return self.meas_at(source, kind) };
        let Some((key, t0)) = self.own_key(source, own) else { return f64::NAN };
        let index = ALL_MEAS.iter().position(|&entry| entry == kind).unwrap();
        if let Some(value) = self
            .azr
            .own_measure
            .borrow()
            .cached
            .iter()
            .find(|cached| cached.key == key)
            .and_then(|cached| cached.readings.get(index))
            .copied()
        {
            return if matches!(kind, Meas::TVmax | Meas::TVmin) { value + t0 } else { value };
        }
        if key.analysis.hi - key.analysis.lo <= BACKGROUND_MEASUREMENT_POINTS {
            let Some(job) = self.own_job(key) else { return f64::NAN };
            let analysis = job.analysis.unwrap_or_else(|| {
                Arc::new(oos_dsp::measure::analyze_refs(job.samples.slice(), job.dt, job.min_amp, job.low, job.high))
            });
            let readings = scalar_readings(&analysis, key.analysis);
            let value = readings[index];
            self.cache_own_result(key, analysis, readings);
            return if matches!(kind, Meas::TVmax | Meas::TVmin) { value + t0 } else { value };
        }
        {
            let mut state = self.azr.own_measure.borrow_mut();
            state.pending.retain(|request| {
                !(request.key.source == source && request.owner == owner && request.kind == kind && request.key != key)
            });
            if !state.pending.iter().any(|request| request.key == key && request.owner == owner && request.kind == kind)
            {
                if state.pending.len() >= SCALAR_LIMIT {
                    state.pending.pop_front();
                }
                state.pending.push_back(Request { key, kind, owner });
            }
        }
        self.schedule_own_measurement();
        f64::NAN
    }

    pub(super) fn schedule_own_measurement(&self) {
        let request = {
            let mut state = self.azr.own_measure.borrow_mut();
            state.pending.retain(|request| self.request_current(*request));
            state.pending.front().copied()
        };
        let Some(request) = request else { return };
        let mut worker = self.azr.ref_measure_worker.borrow_mut();
        let worker = worker.get_or_insert_with(RefMeasureWorker::open);
        if worker.busy {
            return;
        }
        let Some(job) = self.own_job(request.key) else { return };
        if worker.input.try_send(job).is_ok() {
            worker.busy = true;
            worker.submitted += 1;
        }
    }

    pub(super) fn commit_own_measurement(&self, result: RefMeasureResult) -> bool {
        let key = Key { source: result.source, analysis: result.key };
        let accepted = self
            .azr
            .own_measure
            .borrow()
            .pending
            .iter()
            .any(|request| request.key == key && self.request_current(*request));
        self.azr.own_measure.borrow_mut().pending.retain(|request| request.key != key);
        if accepted {
            self.cache_own_result(key, result.analysis, result.readings);
        }
        accepted
    }

    pub(super) fn own_measurement_busy(&self) -> bool {
        self.azr.own_measure.borrow().pending.iter().any(|request| self.request_current(*request))
    }

    pub(in crate::ui) fn meas_number_computing(&self, source: WaveId, kind: Meas) -> bool {
        let Some(own) = self.own_params(kind, source) else { return self.measurement_computing(source) };
        let Some((key, _)) = self.own_key(source, own) else { return false };
        self.azr.own_measure.borrow().pending.iter().any(|request| request.key == key && self.request_current(*request))
    }

    /// Labels only use the matching row analysis; an evicted large analysis may
    /// omit annotations while its numeric value remains cached and available.
    pub(in crate::ui) fn meas_number_analysis(
        &self,
        source: WaveId,
        kind: Meas,
    ) -> Option<(Arc<oos_dsp::measure::Analysis>, f64)> {
        let Some(own) = self.own_params(kind, source) else { return self.meas_analysis(source) };
        let (key, t0) = self.own_key(source, own)?;
        let _ = self.measurement_with_own(source, kind, source);
        self.reusable_own_analysis(key).map(|analysis| (analysis, t0))
    }
}

#[cfg(test)]
mod tests;
