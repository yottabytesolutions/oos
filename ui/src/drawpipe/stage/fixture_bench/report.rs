// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Range, Sample};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stage {
    Core,
    Decode,
    Geometry,
    Fold,
    TriggerSource,
    Trigger,
    ColumnEdges,
    MinMax,
    CodeToRow,
    FusedReduceMap,
    Resample,
    Accumulate,
    Split,
    Writes,
    Reset,
    Place,
    Remainder,
}

impl Stage {
    pub(super) const ALL: [Self; 17] = [
        Self::Core,
        Self::Decode,
        Self::Geometry,
        Self::Fold,
        Self::TriggerSource,
        Self::Trigger,
        Self::ColumnEdges,
        Self::MinMax,
        Self::CodeToRow,
        Self::FusedReduceMap,
        Self::Resample,
        Self::Accumulate,
        Self::Split,
        Self::Writes,
        Self::Reset,
        Self::Place,
        Self::Remainder,
    ];
    pub(super) const PLACEMENT: [Self; 13] = [
        Self::Core,
        Self::Decode,
        Self::Geometry,
        Self::Fold,
        Self::TriggerSource,
        Self::Trigger,
        Self::ColumnEdges,
        Self::MinMax,
        Self::CodeToRow,
        Self::FusedReduceMap,
        Self::Resample,
        Self::Accumulate,
        Self::Split,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Core => "core_estimate",
            Self::Decode => "decode",
            Self::Geometry => "placement_geometry",
            Self::Fold => "measurement_fold",
            Self::TriggerSource => "trigger_source_decode",
            Self::Trigger => "trigger",
            Self::ColumnEdges => "column_edges",
            Self::MinMax => "min_max",
            Self::CodeToRow => "code_to_row",
            Self::FusedReduceMap => "fused_reduce_map",
            Self::Resample => "resample",
            Self::Accumulate => "average_accumulate",
            Self::Split => "split_columns",
            Self::Writes => "tile_writes",
            Self::Reset => "plane_reset",
            Self::Place => "place",
            Self::Remainder => "place_remainder",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Options {
    stage: Option<Stage>,
    repeats: usize,
    cpu: Option<usize>,
}

impl Options {
    pub(super) fn from_env() -> Self {
        let stage = std::env::var("OOS_BENCH_STAGE")
            .ok()
            .map(|name| Stage::ALL.into_iter().find(|stage| stage.name() == name).expect("known OOS_BENCH_STAGE"));
        let repeats = std::env::var("OOS_BENCH_REPEATS").map_or(1, |n| n.parse().expect("integer OOS_BENCH_REPEATS"));
        assert!((1..=1_000_000).contains(&repeats));
        assert!(repeats == 1 || stage != Some(Stage::Remainder), "place_remainder requires a cold pass");
        let cpu = std::env::var("OOS_BENCH_CPU").ok().map(|n| n.parse().expect("integer OOS_BENCH_CPU"));
        Self { stage, repeats, cpu }
    }

    pub(super) fn for_test(stage: Stage, repeats: usize) -> Self {
        Self { stage: Some(stage), repeats, cpu: None }
    }

    pub(super) fn pin(&self) {
        if let Some(cpu) = self.cpu {
            #[cfg(target_os = "linux")]
            oos_hal::system::pin_current_thread(&[cpu]).expect("pin benchmark thread to OOS_BENCH_CPU");
            #[cfg(not(target_os = "linux"))]
            panic!("setting OOS_BENCH_CPU={cpu} requires Linux affinity");
        }
    }

    pub(super) fn repeats(&self) -> usize {
        self.repeats
    }

    pub(super) fn measures(&self, stage: Stage) -> bool {
        self.repeats == 1 || self.stage == Some(stage)
    }

    fn stages(&self) -> impl Iterator<Item = Stage> + '_ {
        Stage::ALL.into_iter().filter(|&stage| self.stage.is_none_or(|only| only == stage))
    }

    pub(super) fn passes(self) -> impl Iterator<Item = Self> {
        let mut stages = Stage::ALL.into_iter().filter(move |&stage| {
            self.repeats > 1 && stage != Stage::Remainder && self.stage.is_none_or(|only| only == stage)
        });
        (self.repeats == 1)
            .then_some(self)
            .into_iter()
            .chain(std::iter::from_fn(move || stages.next().map(|stage| Self { stage: Some(stage), ..self })))
    }

    pub(super) fn report(&self, fixture: &str, range: Range<usize>, channels: [bool; 4], samples: &[Sample]) {
        let active = channels.iter().filter(|&&on| on).count() as u32;
        let cpu = self.cpu.map_or_else(|| "null".to_owned(), |n| n.to_string());
        let fixture = crate::json::J::s(fixture).to_string();
        let mode = if self.repeats == 1 { "cold" } else { "instructions" };
        for stage in self.stages() {
            let records = samples.iter().filter(|sample| sample.calls(stage) != 0).count();
            let calls: usize = samples.iter().map(|sample| sample.calls(stage)).sum();
            assert_eq!(calls, records * self.repeats, "{}", stage.name());
            for (record, sample) in range.clone().zip(samples) {
                let calls = sample.calls(stage);
                let work = sample.work(stage);
                let ns = |elapsed: std::time::Duration| {
                    if self.repeats == 1 && calls != 0 {
                        elapsed.as_nanos().to_string()
                    } else {
                        "null".into()
                    }
                };
                let share = if stage == Stage::Remainder {
                    let place = sample.work(Stage::Place).total();
                    if place.is_zero() {
                        "null".into()
                    } else {
                        (work.total().as_secs_f64() / place.as_secs_f64() * 100.0).to_string()
                    }
                } else {
                    "null".into()
                };
                for (channel, _) in channels.into_iter().enumerate().filter(|(_, on)| *on) {
                    println!(
                        "{{\"fixture\":{fixture},\"stage\":\"{}\",\"mode\":\"{mode}\",\"cpu\":{cpu},\"record\":{record},\"channel\":{},\"repeats\":{},\"calls\":{calls},\"ns_per_record\":{},\"ns_per_record_channel\":{},\"percent_of_place\":{share}}}",
                        stage.name(), channel+1, self.repeats, ns(work.total()), ns(work.lane(channel, active)),
                    );
                }
            }
        }
    }
}
