// SPDX-License-Identifier: GPL-2.0-or-later

use crate::fastdraw::Samples;
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct SpanWork {
    pub(super) samples: usize,
    pub(super) joins: usize,
    pub(super) scattered: usize,
    pub(super) contiguous: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct DotWork {
    pub(super) samples: usize,
    pub(super) probes: usize,
    pub(super) writes: usize,
}

thread_local! {
    static SPANS: Cell<Option<SpanWork>> = const { Cell::new(None) };
    static DOTS: Cell<Option<DotWork>> = const { Cell::new(None) };
}

pub(super) fn spans(scattered: usize, contiguous: usize) {
    if let Some(mut work) = SPANS.get() {
        work.scattered += scattered;
        work.contiguous += contiguous;
        SPANS.set(Some(work));
    }
}

pub(super) struct CountedSamples<'a>(pub(super) &'a [f32]);

impl Samples for CountedSamples<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn at(&self, index: usize) -> f32 {
        let mut work = SPANS.get().expect("counted samples require an active observation");
        work.joins += 1;
        SPANS.set(Some(work));
        self.0.at(index)
    }

    fn extremes(&self, first: usize, end: usize) -> (f32, f32) {
        let mut work = SPANS.get().expect("counted samples require an active observation");
        work.samples += end - first;
        SPANS.set(Some(work));
        self.0.extremes(first, end)
    }
}

pub(super) fn dots(column: DotWork) {
    if let Some(mut work) = DOTS.get() {
        work.samples += column.samples;
        work.probes += column.probes;
        work.writes += column.writes;
        DOTS.set(Some(work));
    }
}

pub(super) fn count_spans(draw: impl FnOnce()) -> SpanWork {
    SPANS.set(Some(SpanWork::default()));
    draw();
    SPANS.take().expect("recording span writes")
}

pub(super) fn count_dots(draw: impl FnOnce()) -> DotWork {
    DOTS.set(Some(DotWork::default()));
    draw();
    DOTS.take().expect("recording dot work")
}
