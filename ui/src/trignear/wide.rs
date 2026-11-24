// SPDX-License-Identifier: GPL-2.0-or-later

use super::{finite_or_level, pruned::Band, sample_bounds, EdgeSearch, SearchResult};
use oos_dsp::edges::Edge;
use std::ops::Range;

/// The serial search geometry, shared by the workers of a deep pairs record.
#[derive(Clone, Copy)]
pub(crate) struct WideSearch {
    pre: f64,
    span: usize,
    width: usize,
    trigger: EdgeSearch,
}

#[derive(Default)]
pub(crate) struct WideShard {
    starts: Range<usize>,
    sums: (f64, f64),
    samples: Vec<f32>,
    edges: Vec<Edge>,
    boundary: Boundary,
}

impl WideSearch {
    pub(super) fn new(pre: f64, span: usize, width: usize, trigger: EdgeSearch) -> Self {
        Self { pre, span, width, trigger }
    }

    fn starts(self, len: usize) -> Range<usize> {
        sample_bounds(len, (self.pre as usize, self.span, self.width))
            .map_or(0..0, |bounds| bounds.start..bounds.end - self.width + 1)
    }

    pub(crate) fn windows(self, len: usize) -> usize {
        if !self.trigger.level.is_finite() || !self.trigger.hysteresis.is_finite() || self.trigger.hysteresis < 0.0 {
            return 0;
        }
        self.starts(len).len()
    }

    pub(crate) fn serial(self, source: &[f32], fits: &dyn Fn(f64) -> bool) -> SearchResult {
        super::smooth_all(source, self.pre, self.trigger, self.span, self.width, fits)
    }

    pub(crate) fn smooth(self, source: &[f32], offsets: Range<usize>, shard: &mut WideShard) {
        let first = self.starts(source.len()).start;
        shard.starts = first + offsets.start..first + offsets.end;
        shard.fill(source, self, None);
        shard.scan(first, self);
    }

    pub(crate) fn resolve<'a>(
        self,
        source: &[f32],
        shards: impl Iterator<Item = &'a mut WideShard>,
        fits: &dyn Fn(f64) -> bool,
    ) -> SearchResult {
        let origin = self.starts(source.len()).start;
        crate::fastdraw::with_scratch(|work| {
            let scratch = &mut work.trigger;
            let mut previous = None;
            let mut carry = Carry::default();
            scratch.edges.clear();
            for shard in shards {
                // Restarting a floating-point sum changes its rounding in general. Check each seam against
                // the serial recurrence and only redo a shard when the provisional sum differs.
                let first = shard.starts.start;
                if let Some(sum) = previous {
                    let next = finite_or_level(source[first + self.width - 1], self.trigger.level);
                    let old = finite_or_level(source[first - 1], self.trigger.level);
                    let expected: f64 = sum + (next - old);
                    if shard.sums.0.to_bits() != expected.to_bits() {
                        shard.fill(source, self, Some(expected));
                        shard.scan(origin, self);
                    }
                }
                previous = Some(shard.sums.1);
                shard.boundary.append(&mut carry, self.trigger.level, &mut scratch.edges);
                scratch.edges.extend_from_slice(&shard.edges);
            }
            scratch.nearest(origin, self.pre, self.trigger, fits)
        })
    }
}

impl WideShard {
    fn fill(&mut self, source: &[f32], search: WideSearch, first_sum: Option<f64>) {
        let width = search.width;
        let sample = |x| finite_or_level(x, search.trigger.level);
        let first = self.starts.start;
        let (leaving, entering) = (first..self.starts.end - 1, first + width..self.starts.end + width - 1);
        let first_sum = first_sum
            .unwrap_or_else(|| source[first..first + width].iter().fold(0.0, |sum, &value| sum + sample(value)));
        let mut sum = first_sum;
        self.samples.clear();
        self.samples.reserve(self.starts.len());
        self.samples.push((sum / width as f64) as f32);
        self.samples.extend(source[leaving].iter().zip(&source[entering]).map(|(&old, &next)| {
            sum += sample(next) - sample(old);
            (sum / width as f64) as f32
        }));
        self.sums = (first_sum, sum);
    }

    fn scan(&mut self, origin: usize, search: WideSearch) {
        let offset = self.starts.start - origin + search.width / 2;
        let samples = self.samples.iter().copied().enumerate().map(|(i, x)| (offset + i, x));
        let trigger = search.trigger;
        let (lo, hi) = crate::fastdraw::min_max(&self.samples);
        if lo.is_finite() && hi.is_finite() && Band::of(lo, hi, trigger) != Band::Mixed {
            // Every value makes the same comparator decisions. Only the endpoints can affect adjacent shards.
            self.boundary = Boundary::of(samples.clone().take(1).chain(samples.rev().take(1)), trigger);
            self.edges.clear();
        } else {
            self.boundary = Boundary::of(samples.clone(), trigger);
            oos_dsp::edges::trigger_edges_indexed_into(samples, trigger.level, trigger.hysteresis, &mut self.edges);
        }
    }
}

type Sample = (usize, f32);

#[derive(Default)]
struct Carry {
    armed: [bool; 2],
    previous: Option<Sample>,
}

/// Starting unarmed can hide one edge in the prefix. After the first arm or fire of a
/// direction, its state is independent of the preceding shard.
#[derive(Default)]
struct Boundary {
    comparators: [ComparatorBoundary; 2],
    last: Option<Sample>,
}

#[derive(Default)]
struct ComparatorBoundary {
    fire: Option<(Option<Sample>, Sample)>,
    armed: Option<bool>,
}

impl Boundary {
    fn of(samples: impl DoubleEndedIterator<Item = Sample> + Clone, trigger: EdgeSearch) -> Self {
        let samples = samples.filter(|&(_, x)| x.is_finite());
        let last = samples.clone().next_back();
        let lo = trigger.level - trigger.hysteresis;
        let hi = trigger.level + trigger.hysteresis;
        let comparators = [true, false].map(|rising| {
            let decision = |x| {
                if (rising && x < lo) || (!rising && x > hi) {
                    Some(true)
                } else if (rising && x >= trigger.level) || (!rising && x <= trigger.level) {
                    Some(false)
                } else {
                    None
                }
            };
            let mut previous = None;
            let fire = samples
                .clone()
                .find_map(|sample| {
                    let result = decision(sample.1).map(|armed| (!armed).then_some((previous, sample)));
                    previous = Some(sample);
                    result
                })
                .flatten();
            let armed = samples.clone().rev().find_map(|(_, x)| decision(x));
            ComparatorBoundary { fire, armed }
        });
        Self { comparators, last }
    }

    fn append(&self, carry: &mut Carry, level: f32, edges: &mut Vec<Edge>) {
        // With nonnegative hysteresis, at most one comparator arrives armed. Its
        // pending edge precedes local edges: the opposite rail would fire it first.
        for (direction, (end, armed)) in self.comparators.iter().zip(&mut carry.armed).enumerate() {
            if let Some((prev, (i, x))) = end.fire.filter(|_| *armed) {
                let t = prev
                    .or(carry.previous)
                    .map_or(i as f64, |(j, p)| j as f64 + (level - p) as f64 / (x - p) as f64 * (i - j) as f64);
                edges.push(Edge { t, rising: direction == 0 });
            }
            *armed = end.armed.unwrap_or(*armed);
        }
        carry.previous = self.last.or(carry.previous);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sharded(source: &[f32], search: WideSearch, count: usize, fits: &dyn Fn(f64) -> bool) -> SearchResult {
        let n = search.windows(source.len());
        let mut shards: Vec<_> = (0..count).map(|_| WideShard::default()).collect();
        for (i, shard) in shards.iter_mut().enumerate() {
            search.smooth(source, n * i / count..n * (i + 1) / count, shard);
        }
        search.resolve(source, shards.iter_mut(), fits)
    }

    #[test]
    fn wide_shards_preserve_the_first_of_two_equidistant_edges() {
        let mut source = [-1.0; 12];
        source[4] = 1.0;
        let trigger = EdgeSearch { level: 0.0, hysteresis: 0.25, rising: true, either: true };
        let search = WideSearch::new(4.0, 10, 1, trigger);
        let filters: [&dyn Fn(f64) -> bool; 2] = [&|_| true, &|t| t > 4.0];
        for fits in filters {
            assert_eq!(sharded(&source, search, 3, fits), search.serial(&source, fits));
        }
        assert_eq!(sharded(&source, search, 3, &|_| true).0, Some(3.5));
    }

    #[test]
    fn wide_shards_preserve_rounding_and_comparator_history() {
        let mut source = [-1.0; 257];
        // Subtracting the large initial sample cancels the first rounded sum, so
        // later windows cannot be restarted independently.
        source[0] = 1e30;
        source[64..192].fill(-0.0625);
        for (i, x) in source[192..].iter_mut().enumerate() {
            *x = [0.5, 0.25, -0.5, -0.25, f32::NAN][i % 5];
        }
        for width in [1, 2, 11, 64] {
            for (rising, either) in [(true, false), (false, false), (true, true)] {
                let trigger = EdgeSearch { level: 0.0, hysteresis: 0.25, rising, either };
                let search = WideSearch::new(160.0, 180, width, trigger);
                for count in [2, 3, 4, 5] {
                    let fits = |t| t >= 48.0;
                    assert_eq!(sharded(&source, search, count, &fits), search.serial(&source, &fits));
                }
            }
        }
    }

    #[test]
    fn single_comparator_bands_keep_edges_at_shard_boundaries() {
        let mut source = [-1.0; 240];
        for (i, x) in source.iter_mut().enumerate() {
            let band = [-0.75, -0.20, 0.18, 0.7, 0.22, -0.19][i / 40];
            *x = band + (i % 7) as f32 * 0.003;
        }
        for rising in [true, false] {
            let trigger = EdgeSearch { level: 0.0, hysteresis: 0.25, rising, either: false };
            for (pre, bounds) in [(80, (75.0, 90.0)), (200, (195.0, 210.0))] {
                let search = WideSearch::new(pre as f64, 240, 1, trigger);
                let fits = |t| (bounds.0..bounds.1).contains(&t);
                let serial = search.serial(&source, &fits);
                assert!(serial.1.is_some());
                assert_eq!(sharded(&source, search, 6, &fits), serial);
            }
        }
    }
}
