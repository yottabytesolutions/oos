// SPDX-License-Identifier: GPL-2.0-or-later

//! Pure SINGLE completion and poll decisions shared by the worker and HAL fixture.

use oos_hal::dho::seq::Settings;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Poll {
    Hold,
    Wait,
    Acquire,
}

pub(super) fn progress(
    settings: &Settings,
    done: Option<(u64, u64)>,
    pair: (u64, u64),
    hold: bool,
    published: u32,
) -> (Option<(u64, u64)>, Poll) {
    let done = if published > 0 && settings.single { Some(pair) } else { done };
    let poll = if hold {
        Poll::Hold
    } else if !super::single_wants_more(settings, done, pair) {
        Poll::Wait
    } else {
        Poll::Acquire
    };
    (done, poll)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hold_precedes_both_same_pair_wait_and_changed_pair_retry() {
        let settings = Settings { single: true, ..Default::default() };
        for done in [None, Some((1, 2)), Some((3, 4))] {
            assert_eq!(progress(&settings, done, (1, 2), true, 0), (done, Poll::Hold));
        }
        assert_eq!(progress(&settings, Some((1, 2)), (1, 2), false, 0).1, Poll::Wait);
        assert_eq!(progress(&settings, Some((1, 2)), (3, 4), false, 0).1, Poll::Acquire);
        assert_eq!(progress(&settings, None, (1, 2), false, 0), (None, Poll::Acquire));
        assert_eq!(progress(&settings, None, (1, 2), false, 1), (Some((1, 2)), Poll::Wait));
    }
}
