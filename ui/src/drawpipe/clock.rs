// SPDX-License-Identifier: GPL-2.0-or-later

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Default)]
pub(crate) struct StageClock {
    first_trigger_at: Option<Instant>,
    admitted: bool,
    #[cfg(test)]
    test_dequeue: Option<Instant>,
    #[cfg(test)]
    test_pool: Option<Duration>,
    read_at: Option<Instant>,
    submitted: Option<Instant>,
    dequeued: Option<Instant>,
    deposited: Option<Instant>,
    published: Option<Instant>,
    // Slot custody can move to the render owner early. First-use claim begins at composition.
    staged: Option<Instant>,
    claimed: Option<Instant>,
}

impl From<Instant> for StageClock {
    fn from(read_at: Instant) -> Self {
        Self::read(None, read_at)
    }
}

impl StageClock {
    pub(crate) fn read(first_trigger_at: Option<Instant>, read_at: Instant) -> Self {
        Self { first_trigger_at, read_at: Some(read_at), ..Self::default() }
    }

    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn with_admission(mut self) -> Self {
        self.admitted = true;
        self
    }

    #[cfg(test)]
    pub(crate) fn dequeue_for_test(mut self, at: Instant) -> Self {
        self.test_dequeue = Some(at);
        self
    }

    #[cfg(test)]
    pub(crate) fn pool_for_test(mut self, cost: Duration) -> Self {
        self.test_pool = Some(cost);
        self
    }

    #[cfg(test)]
    pub(super) fn pool_override(self) -> Option<Duration> {
        self.test_pool
    }

    pub(super) fn admitted(self) -> bool {
        self.admitted
    }

    pub(super) fn read_at(self) -> Instant {
        self.read_at.expect("submitted batches carry the read completion time")
    }

    pub(super) fn submit(&mut self) {
        self.submitted = Some(Instant::now());
    }

    pub(super) fn dequeue(self, at: Instant) -> Self {
        #[cfg(test)]
        let at = self.test_dequeue.unwrap_or(at);
        Self { dequeued: Some(at), ..self }
    }

    pub(super) fn publish(&mut self) {
        self.published = Some(Instant::now());
    }

    pub(super) fn deposit(&mut self) {
        self.deposited = Some(Instant::now());
    }

    pub(super) fn outcome(self, at: Instant) -> [Option<Duration>; 7] {
        let elapsed = |from: Option<Instant>, to: Option<Instant>| to?.checked_duration_since(from?);
        [
            elapsed(self.read_at, self.submitted),
            elapsed(self.submitted, self.dequeued),
            elapsed(self.dequeued, self.deposited),
            elapsed(self.deposited, self.published),
            elapsed(self.published, self.claimed),
            elapsed(self.claimed, Some(at)),
            elapsed(self.first_trigger_at, Some(at)),
        ]
    }
}

/// At capacity retain the oldest batches and the latest one. A new batch can never replace the oldest.
const CAPACITY: usize = 4;

#[derive(Default)]
pub(crate) struct BatchClocks {
    clocks: [StageClock; CAPACITY],
    len: usize,
}

impl BatchClocks {
    pub(crate) fn push(&mut self, clock: StageClock) -> bool {
        let truncated = self.len == CAPACITY;
        self.clocks[self.len.min(CAPACITY - 1)] = clock;
        self.len += usize::from(!truncated);
        truncated
    }

    pub(crate) fn published(&mut self) {
        self.stamp(|clock, at| {
            clock.published = Some(at);
            clock.staged = None;
            clock.claimed = None;
        });
    }

    pub(crate) fn staged(&mut self) {
        self.stamp(|clock, at| clock.staged = Some(at));
    }

    pub(crate) fn claimed(&mut self, at: Instant) {
        for clock in &mut self.clocks[..self.len] {
            clock.claimed.get_or_insert(at);
        }
    }

    pub(crate) fn longest_wait(&self, start: Instant) -> Option<Duration> {
        self.clocks[..self.len]
            .iter()
            .filter_map(|clock| clock.staged.or(clock.claimed))
            .map(|at| start.saturating_duration_since(at))
            .max()
    }

    fn stamp(&mut self, stamp: impl Fn(&mut StageClock, Instant)) {
        if self.len > 0 {
            let now = Instant::now();
            for clock in &mut self.clocks[..self.len] {
                stamp(clock, now);
            }
        }
    }

    pub(super) fn outcome(&self, at: Instant) -> impl Iterator<Item = [Option<Duration>; 7]> + '_ {
        self.clocks[..self.len].iter().map(move |clock| clock.outcome(at))
    }

    pub(super) fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_clocks_retain_both_ends() {
        let start = Instant::now();
        let mut clocks = BatchClocks::default();
        for i in 0..10 {
            assert_eq!(clocks.push((start + Duration::from_micros(i as u64)).into()), i >= CAPACITY);
        }
        assert_eq!(clocks.clocks.map(StageClock::read_at).map(|at| (at - start).as_micros()), [0, 1, 2, 9]);
    }

    #[test]
    fn staged_wait_is_separate_from_the_picture_claim() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let mut clocks = BatchClocks::default();
        clocks.push(StageClock { published: Some(at(1)), staged: Some(at(2)), ..StageClock::from(start) });

        assert_eq!(clocks.longest_wait(at(20)), Some(Duration::from_millis(18)));
        assert_eq!(clocks.outcome(at(20)).next().unwrap()[4..6], [None, None]);

        clocks.claimed(at(20));
        clocks.claimed(at(21));
        let published_to_claim = Some(Duration::from_millis(19));
        let claim_to_compose = Some(Duration::from_millis(3));
        assert_eq!(clocks.outcome(at(23)).next().unwrap()[4..6], [published_to_claim, claim_to_compose]);
    }
}
