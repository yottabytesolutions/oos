// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    dot, fixture_stage, kernel, kernel_edge, position, Taps, COEFFICIENT_UNIT, PHASES, POSITION_BITS, POSITION_UNIT,
    TAPS,
};
use crate::fastdraw::Codes;
use std::ops::Range;

/// Point samples for Average's screen grid. Each linear point reads at most two DMA words.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AverageGrid {
    points: usize,
    first: f64,
    dt: f64,
    sample_dt: f64,
    trigger: f64,
}

impl AverageGrid {
    pub(crate) fn new((points, window, first): (usize, f64, f64), sample_dt: f64, trigger: f64) -> Self {
        Self { points, first, dt: window / points as f64, sample_dt, trigger }
    }

    fn lanczos(self) -> bool {
        self.sample_dt > 2.0 * self.dt
    }

    /// Short sinc records can touch both ends in one kernel and use the decoded interpolator.
    pub(crate) fn supports_lanczos_edges(self, samples: usize) -> bool {
        !self.lanczos() || samples >= TAPS
    }

    pub(crate) fn fill(self, _channel: usize, codes: &Codes<'_>, out: &mut Vec<f32>) {
        out.clear();
        if codes.n < 2 {
            return;
        }
        out.reserve(self.points);
        let volts = Voltage::new(codes);
        if self.lanczos() {
            fixture_stage!(AverageLanczos, _channel, self.lanczos_grid(codes, volts, out));
        } else {
            fixture_stage!(AverageLinear, _channel, self.linear_grid(codes, volts, out));
        }
    }

    fn linear_grid(self, codes: &Codes<'_>, volts: Voltage, out: &mut Vec<f32>) {
        let origin = self.trigger + self.first / self.sample_dt;
        let step = self.dt / self.sample_dt;
        let visible = self.visible(codes.n, |k| self.trigger + (self.first + k as f64 * self.dt) / self.sample_dt);
        out.resize(visible.start, f32::NAN);
        if !visible.is_empty() {
            let x = (origin + visible.start as f64 * step).clamp(0.0, (codes.n - 1) as f64);
            let first = (x as usize).min(codes.n - 2);
            // Start the Q36 walk at the first pair. Rounding the step contributes at most
            // 3600 / 2^37 sample of drift, under 0.002 ADC count even across both rails.
            let grid = (first, position(x - first as f64), position(step), visible.len());
            match codes.stride {
                1 => linear::<1>(codes, grid, volts, out),
                2 => linear::<2>(codes, grid, volts, out),
                _ => linear::<4>(codes, grid, volts, out),
            }
        }
        out.resize(self.points, f32::NAN);
    }

    fn lanczos_grid(self, codes: &Codes<'_>, volts: Voltage, out: &mut Vec<f32>) {
        let origin = self.trigger + self.first / self.sample_dt;
        let step = self.dt / self.sample_dt;
        let at = |k| origin + k as f64 * step;
        let mut visible = self.visible(codes.n, at);
        out.resize(visible.start, f32::NAN);
        if !visible.is_empty() {
            let reciprocal = step.recip();
            let mut base = at(visible.start) as usize;
            let mut taps: Taps = std::array::from_fn(|j| centered_at(codes, base + j, 7));
            while visible.start < visible.end {
                let phases = kernel(kernel_edge(base, codes.n));
                let end = run_end(visible.clone(), base, origin, reciprocal, at);
                let sample = base as f64;
                out.extend((visible.start..end).map(|k| {
                    let phase = ((at(k) - sample) * PHASES as f64).round() as usize;
                    volts.convert(dot(&taps, &phases[phase]) as f32 * (1.0 / COEFFICIENT_UNIT as f32))
                }));
                if end == visible.end {
                    break;
                }
                // The grid advances by less than half a sample. Keep 15 taps and fetch one DMA word.
                base += 1;
                taps.rotate_left(1);
                taps[TAPS - 1] = centered_at(codes, base + 8, 0);
                visible.start = end;
            }
        }
        out.resize(self.points, f32::NAN);
    }

    fn visible(self, samples: usize, at: impl Fn(usize) -> f64) -> Range<usize> {
        // Test the actual legacy coordinates at the boundaries to keep the NaN mask identical.
        let last = (samples - 1) as f64;
        let first = if at(0) >= 0.0 { 0 } else { partition(0..self.points, |k| at(k) < 0.0) };
        let end =
            if at(self.points - 1) <= last { self.points } else { partition(first..self.points, |k| at(k) <= last) };
        first..end
    }
}

fn run_end(range: Range<usize>, base: usize, origin: f64, reciprocal: f64, at: impl Fn(usize) -> f64) -> usize {
    let next = (base + 1) as f64;
    let estimate = (((next - origin) * reciprocal).ceil() as usize).clamp(range.start + 1, range.end);
    // Check the estimate using the same coordinates and rounding as the legacy Lanczos call.
    if at(estimate - 1) >= next {
        partition(range.start + 1..estimate, |k| at(k) < next)
    } else if estimate < range.end && at(estimate) < next {
        partition(estimate + 1..range.end, |k| at(k) < next)
    } else {
        estimate
    }
}

fn partition(mut range: Range<usize>, before: impl Fn(usize) -> bool) -> usize {
    while range.start < range.end {
        let mid = range.start.midpoint(range.end);
        if before(mid) {
            range.start = mid + 1;
        } else {
            range.end = mid;
        }
    }
    range.start
}

#[derive(Clone, Copy)]
struct Voltage {
    gain: f32,
    center: f32,
}

impl Voltage {
    fn new(codes: &Codes<'_>) -> Self {
        let gain = codes.corrected.map_or(codes.affine.0, |cal| (cal.scale / 7500.0) as f32);
        Self { gain, center: codes.volts(32768) }
    }

    #[inline]
    fn convert(self, centered_code: f32) -> f32 {
        centered_code.mul_add(self.gain, self.center)
    }
}

fn centered_at(codes: &Codes<'_>, index: usize, before: usize) -> i16 {
    index.checked_sub(before).filter(|&i| i < codes.n).map_or(0, |i| (codes.at_code(i) ^ 0x8000) as i16)
}

fn linear<const STRIDE: usize>(
    codes: &Codes<'_>,
    (first, mut x, step, count): (usize, i64, i64, usize),
    volts: Voltage,
    out: &mut Vec<f32>,
) {
    let words = codes.words.expect("the Average route validated aligned words");
    let words = &words[first * STRIDE + codes.word..];
    let last = codes.n - 2 - first;
    let cal = codes.corrected.as_ref();
    let sample = |i| {
        let word = words[i * STRIDE];
        f32::from(cal.map_or(word, |c| c.code(first + i, word))) - 32768.0
    };
    let mut base = 0;
    let (mut left, mut right) = (sample(0), sample(1));
    out.extend((0..count).map(|_| {
        let i = ((x >> POSITION_BITS) as usize).min(last);
        if i != base {
            left = if i == base + 1 { right } else { sample(i) };
            right = sample(i + 1);
            base = i;
        }
        let fraction = (x - ((i as i64) << POSITION_BITS)) as f32 * (1.0 / POSITION_UNIT as f32);
        x += step;
        volts.convert(fraction.mul_add(right - left, left))
    }));
}
