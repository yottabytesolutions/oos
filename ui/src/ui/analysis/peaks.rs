// SPDX-License-Identifier: GPL-2.0-or-later
//! Bounded UI-owned peak identities. Time is supplied by the UI clock, never read here.

use super::{spectrum, WaveId, Window};
use spectrum::Marker;

pub(in crate::ui) const MAX: usize = 8;
const APPEAR_S: f64 = 1.0;
const RELEASE_S: f64 = 1.0;
const MATCH_BINS: f64 = 2.0;
const MATCH_RBW: f64 = 0.5;
const LEVEL_DB: f64 = 1.0;
const CAPACITY: usize = 2 * MAX;

type Key = ((WaveId, Window, usize), f64, f64, f64);

#[derive(Clone, Copy)]
struct Track {
    value: Marker,
    since: f64,
    missing: Option<f64>,
    number: usize,
}

#[derive(Clone, Default)]
pub(in crate::ui) struct Table {
    tracks: [Option<Track>; CAPACITY],
    order: [usize; MAX],
    len: usize,
    key: Option<Key>,
    last: Option<f64>,
    by_level: bool,
}

impl Table {
    pub(super) fn refresh(
        &mut self,
        raw: &[Marker],
        now: f64,
        sp: &spectrum::Spectrum,
        source: (WaveId, Window, usize),
        by_level: bool,
    ) {
        let key = (source, sp.f0, sp.df, sp.rbw);
        if self.key != Some(key) || self.last.is_some_and(|last| now < last) {
            *self = Self::default();
            self.key = Some(key);
        }
        self.last = Some(now);
        self.update(raw, now, (MATCH_BINS * sp.df).max(MATCH_RBW * sp.rbw), by_level);
    }

    pub(in crate::ui) fn rows(&self) -> impl Iterator<Item = (usize, Marker, bool)> + '_ {
        self.order[..self.len].iter().filter_map(|&i| self.tracks[i].map(|t| (t.number, t.value, t.missing.is_some())))
    }

    /// Timers also advance when STOP or a slow worker leaves the displayed spectrum unchanged.
    pub(in crate::ui) fn tick(&mut self, now: f64, by_level: bool) {
        if self.last.is_some_and(|last| now < last) {
            *self = Self::default();
            return;
        }
        self.last = Some(now);
        let before = self.tracks.map(|t| t.is_some_and(|t| t.number > 0));
        self.expire(now);
        self.finish(now, by_level, before);
    }

    fn expire(&mut self, now: f64) {
        for t in &mut self.tracks {
            if t.is_some_and(|t| t.missing.is_some_and(|at| now - at >= RELEASE_S)) {
                *t = None;
            }
        }
    }

    fn update(&mut self, raw: &[Marker], now: f64, tolerance: f64, by_level: bool) {
        let before = self.tracks.map(|t| t.is_some_and(|t| t.number > 0));
        self.expire(now);
        let raw = &raw[..raw.len().min(MAX)];
        let mut used = [false; MAX];
        let mut seen = [false; CAPACITY];
        // Confirmed identities have first choice; each detection can belong to only one track.
        for confirmed in [true, false] {
            self.match_nearest(raw, tolerance, confirmed, &mut used, &mut seen);
        }
        for (i, t) in self.tracks.iter_mut().enumerate() {
            match t {
                Some(t) if seen[i] => t.missing = None,
                Some(t) if t.number > 0 => {
                    t.missing.get_or_insert(now);
                }
                _ => *t = None,
            }
        }
        for (_, &value) in raw.iter().enumerate().filter(|(i, m)| !used[*i] && valid(**m)) {
            if let Some(slot) = self.tracks.iter_mut().find(|t| t.is_none()) {
                *slot = Some(Track { value, since: now, missing: None, number: 0 });
            }
        }
        self.finish(now, by_level, before);
    }

    fn finish(&mut self, now: f64, by_level: bool, before: [bool; CAPACITY]) {
        self.promote(now);
        let changed = before != self.tracks.map(|t| t.is_some_and(|t| t.number > 0));
        self.rebuild_order();
        self.sort(by_level, self.by_level != by_level || (changed && !by_level));
        self.by_level = by_level;
        if changed && !by_level {
            self.renumber();
        }
    }

    fn match_nearest(
        &mut self,
        raw: &[Marker],
        tolerance: f64,
        confirmed: bool,
        used: &mut [bool; MAX],
        seen: &mut [bool; CAPACITY],
    ) {
        loop {
            let mut best = None;
            let mut distance = f64::INFINITY;
            for (i, t) in self.tracks.iter().enumerate() {
                let Some(t) = t.filter(|t| (t.number > 0) == confirmed && !seen[i]) else { continue };
                for (j, m) in raw.iter().enumerate().filter(|(j, m)| !used[*j] && valid(**m)) {
                    let d = (t.value.f - m.f).abs();
                    if d <= tolerance && d < distance {
                        distance = d;
                        best = Some((i, j));
                    }
                }
            }
            let Some((i, j)) = best else { break };
            if let Some(t) = &mut self.tracks[i] {
                t.value = raw[j];
            }
            used[j] = true;
            seen[i] = true;
        }
    }

    fn promote(&mut self, now: f64) {
        let mut numbers = [false; MAX + 1];
        for t in self.tracks.iter().flatten() {
            numbers[t.number] = true;
        }
        for t in self.tracks.iter_mut().flatten().filter(|t| t.number == 0 && now - t.since >= APPEAR_S) {
            if let Some(n) = (1..=MAX).find(|&n| !numbers[n]) {
                t.number = n;
                numbers[n] = true;
            }
        }
    }

    fn rebuild_order(&mut self) {
        let old = self.order;
        let old_len = self.len;
        self.len = 0;
        for i in old[..old_len].iter().copied().chain(0..CAPACITY) {
            if self.tracks[i].is_some_and(|t| t.number > 0) && !self.order[..self.len].contains(&i) {
                self.order[self.len] = i;
                self.len += 1;
            }
        }
    }

    fn sort(&mut self, by_level: bool, force: bool) {
        if !by_level && !force {
            return;
        }
        for end in (1..self.len).rev() {
            for j in 0..end {
                let (Some(a), Some(b)) = (self.tracks[self.order[j]], self.tracks[self.order[j + 1]]) else { continue };
                let swap = if by_level {
                    b.value.dbv > a.value.dbv + if force { 0.0 } else { LEVEL_DB }
                } else {
                    a.value.f > b.value.f
                };
                if swap {
                    self.order.swap(j, j + 1);
                }
            }
        }
    }

    fn renumber(&mut self) {
        for (j, &i) in self.order[..self.len].iter().enumerate() {
            if let Some(t) = &mut self.tracks[i] {
                t.number = j + 1;
            }
        }
    }
}

fn valid(m: Marker) -> bool {
    m.f.is_finite() && m.f > 0.0 && m.dbv.is_finite()
}

#[cfg(test)]
mod tests;
