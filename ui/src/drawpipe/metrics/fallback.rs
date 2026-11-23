// SPDX-License-Identifier: GPL-2.0-or-later

use std::{
    cell::Cell,
    fmt::Write,
    sync::atomic::{AtomicU64, Ordering::Relaxed},
};

macro_rules! causes {
    ($($cause:ident => $label:literal),+ $(,)?) => {
        #[derive(Clone, Copy)]
        pub(crate) enum Cause { $($cause),+ }
        const LABELS: &[&str] = &[$($label),+];
    };
}

causes! {
    NoAdapter => "no_adapter",
    NoWords => "no_words",
    InvalidLayout => "invalid_layout",
    InvalidGeometry => "invalid_geometry",
    ShortInterpolation => "short_interpolation",
    InterpolationTriggerFrame => "interpolation_trigger_frame",
    DecodedLines => "decoded_lines",
    DecodedPairs => "decoded_pairs",
    ScalarPairWords => "scalar_pair_words",
    PairRowTable => "pair_row_table",
    EnvelopeRowTable => "envelope_row_table",
    ScalarRowTable => "scalar_row_table",
    DotRowTable => "dot_row_table",
    DecodedDots => "decoded_dots",
    DotsRedecode => "dots_redecode",
    BoxcarTriggerLane => "boxcar_trigger_lane",
    BoxcarDecoded => "boxcar_decoded",
    BoxcarRowTable => "boxcar_row_table",
    BoxcarRounding => "boxcar_rounding",
    BoxcarVoltageSum => "boxcar_voltage_sum",
    DecodedSparse => "decoded_sparse",
    DecodedScreen => "decoded_screen",
    SplitDecodedScreen => "split_decoded_screen",
    TriggerFromCodes => "trigger_from_codes",
    TriggerFromVolts => "trigger_from_volts",
    ZoomRecordUnsupported => "zoom_record_unsupported",
    ZoomReducedGeometry => "zoom_reduced_geometry",
    ZoomDecoded => "zoom_decoded",
    ZoomHeaderTrigger => "zoom_header_trigger",
    ZoomHeaderTriggerMiss => "zoom_header_trigger_miss",
    ZoomSharedExtrema => "zoom_shared_extrema",
    ZoomLaneExtrema => "zoom_lane_extrema",
    ZoomLinear => "zoom_linear",
    ZoomSinc => "zoom_sinc",
    MeasurementDecode => "measurement_decode",
    ZoomMeasurementDecode => "zoom_measurement_decode",
    AveragePairs => "average_pairs",
    DirectRecord => "direct_record",
}

const _: () = assert!(LABELS.len() <= u64::BITS as usize);

// Each job takes its own marks. Placement joins column marks, then painting joins band marks.
thread_local! { static MARKS: Cell<u64> = const { Cell::new(0) }; }

impl Cause {
    pub(crate) fn mark(self) {
        merge(self.bit());
    }
    pub(crate) fn when(self, condition: bool) {
        if condition {
            self.mark();
        }
    }
    pub(crate) fn absent<T>(self, value: Option<T>) -> Option<T> {
        self.when(value.is_none());
        value
    }
    pub(in crate::drawpipe) fn bit(self) -> u64 {
        1 << self as usize
    }
}

pub(crate) fn merge(bits: u64) {
    MARKS.set(MARKS.get() | bits);
}

pub(crate) fn capture<T>(work: impl FnOnce() -> T) -> (T, u64) {
    let outer = MARKS.replace(0);
    (work(), MARKS.replace(outer))
}

pub(crate) fn direct<T>(work: impl FnOnce() -> T) -> T {
    let (out, marks) = capture(work);
    crate::drawpipe::counters().routes.fallbacks.add(marks);
    out
}

pub(crate) struct Counters([AtomicU64; LABELS.len()]);

impl Default for Counters {
    fn default() -> Self {
        Self([const { AtomicU64::new(0) }; LABELS.len()])
    }
}

impl Counters {
    pub(in crate::drawpipe) fn add(&self, mut causes: u64) {
        while causes != 0 {
            self.0[causes.trailing_zeros() as usize].fetch_add(1, Relaxed);
            causes &= causes - 1;
        }
    }

    pub(in crate::drawpipe) fn write(&self, text: &mut String) {
        text.push_str(" fallback.v1");
        for (label, count) in self.entries() {
            let _ = write!(text, " {label}={count}");
        }
    }

    pub(in crate::drawpipe) fn entries(&self) -> impl Iterator<Item = (&'static str, u64)> + '_ {
        LABELS.iter().copied().zip(self.0.iter().map(|count| count.load(Relaxed)))
    }
}

#[cfg(test)]
#[test]
fn duplicate_causes_across_workers_count_once_per_record() {
    use Cause::{BoxcarRounding, TriggerFromVolts};
    let (_, marks) = capture(|| {
        BoxcarRounding.mark();
        let shard = std::thread::spawn(|| {
            capture(|| {
                BoxcarRounding.mark();
                TriggerFromVolts.mark();
            })
            .1
        });
        merge(shard.join().unwrap());
        TriggerFromVolts.mark();
    });
    let counters = crate::drawpipe::Counters::default();
    counters.routes.fallbacks.add(marks);
    counters.routes.fallbacks.add(TriggerFromVolts.bit());
    let state = counters.pipeline_text(std::time::Duration::ZERO, 0);
    let section = state.split_once(" fallback.v1 ").unwrap().1;
    assert!(section.contains("boxcar_rounding=1 "));
    assert!(section.contains("trigger_from_volts=2 "));
    assert!(section.contains("no_words=0 "));
}
