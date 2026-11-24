// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

#[cfg(any(target_os = "linux", test))]
pub(crate) struct SourceBatch {
    pub(crate) arm: Duration,
    pub(crate) fill: Duration,
    pub(crate) read: Duration,
    pub(crate) readout: Duration,
    pub(crate) read_at: Instant,
    pub(crate) records: u32,
    pub(crate) serial: bool,
    pub(crate) first_trigger: Duration,
    pub(crate) grant: u32,
    pub(crate) bytes: usize,
}

#[derive(Default)]
pub(super) struct Samples {
    window: Window<10>,
    waits: u64,
    serial: u64,
    captured_discarded: u64,
    uncorrected: u64,
    model: [u64; 3],
    #[cfg(any(target_os = "linux", test))]
    pending: Option<Duration>,
    #[cfg(any(target_os = "linux", test))]
    last_read: Option<Instant>,
}

impl Samples {
    #[cfg(target_os = "linux")]
    fn capture_duty(&self, since: Instant) -> Option<f64> {
        if self.last_read.is_none_or(|last| last <= since) || self.window.quantiles(5)[0] == 0 {
            return None;
        }
        Some(self.window.quantiles(6)[0] as f64 / 1_000_000.0)
    }

    pub(super) fn write(&self, text: &mut String, budget: Duration) {
        let [p50, p90] = self.window.quantiles(0);
        let _ = write!(
            text,
            " source.v1 wait_admission={},{:.3}/{:.3}",
            self.waits,
            p50 as f64 / 1000.0,
            p90 as f64 / 1000.0
        );
        for (column, name) in ["arm", "fill", "read", "readout", "records_per_s", "duty"].into_iter().enumerate() {
            let [p50, p90] = self.window.quantiles(column + 1);
            let unit = if column == 5 { 1_000_000.0 } else { 1000.0 };
            let _ = write!(text, " {name}={:.3}/{:.3}", p50 as f64 / unit, p90 as f64 / unit);
        }
        let _ = write!(text, " budget_ms={} source.serial={}", budget.as_millis(), self.serial);
        let [p50, p90] = self.window.quantiles(7);
        let _ = write!(text, " first_trigger={:.3}/{:.3}", p50 as f64 / 1000.0, p90 as f64 / 1000.0);
        for (column, name) in [(8, "grant"), (9, "bytes")] {
            let [p50, p90] = self.window.quantiles(column);
            let _ = write!(text, " {name}={p50}/{p90}");
        }
        let _ = write!(text, " captured_discarded={} uncorrected={}", self.captured_discarded, self.uncorrected);
        let [work, fixed, workers] = self.model;
        let _ = write!(text, " work_per_record_ns={work} fixed_cost_ns={fixed} parallelism={workers}");
    }
}

#[cfg(target_os = "linux")]
impl Metrics {
    pub(crate) fn capture_duty(&self, identity: Identity, since: Instant) -> Option<f64> {
        self.current(Some(identity))?.source.capture_duty(since)
    }
}

#[cfg(any(target_os = "linux", test))]
impl Metrics {
    pub(in crate::drawpipe) fn source_model(&self, identity: Identity, model: [u64; 3]) {
        if let Some(mut current) = self.current(Some(identity)) {
            current.source.model = model;
        }
    }

    pub(crate) fn source_wait(&self, identity: Identity, elapsed: Duration) {
        if let Some(mut current) = self.current(Some(identity)) {
            let source = &mut current.source;
            source.waits += 1;
            *source.pending.get_or_insert(Duration::ZERO) += elapsed;
        }
    }

    pub(crate) fn source_uncorrected(&self, identity: Identity, records: usize) {
        if let Some(mut current) = self.current(Some(identity)) {
            current.source.uncorrected += records as u64;
        }
    }

    pub(crate) fn source_capture_discarded(&self, records: u32) {
        // A settings change can advance the identity while the old capture waits.
        oos_hal::sync::lock(&self.0).source.captured_discarded += u64::from(records);
    }

    pub(crate) fn source_batch(&self, identity: Identity, batch: SourceBatch) {
        if let Some(mut current) = self.current(Some(identity)) {
            let source = &mut current.source;
            source.serial += u64::from(batch.serial);
            let cycle =
                source.last_read.replace(batch.read_at).and_then(|last| batch.read_at.checked_duration_since(last));
            let cycle = cycle.filter(|elapsed| !elapsed.is_zero()).map(|elapsed| elapsed.as_secs_f64());
            let rate = cycle.map(|secs| (f64::from(batch.records) / secs * 1000.0).round() as u64);
            // The first-trigger poll can run after prearm. Its observed fill belongs to this read cycle.
            let duty = cycle.map(|secs| ((batch.fill.as_secs_f64() / secs) * 1_000_000.0).round() as u64);
            let micros = |elapsed| nanos(elapsed) / 1000;
            source.window.add([
                source.pending.take().map(micros),
                Some(micros(batch.arm)),
                Some(micros(batch.fill)),
                Some(micros(batch.read)),
                Some(micros(batch.readout)),
                rate,
                duty,
                Some(micros(batch.first_trigger)),
                Some(u64::from(batch.grant)),
                Some(batch.bytes as u64),
            ]);
        }
    }
}
