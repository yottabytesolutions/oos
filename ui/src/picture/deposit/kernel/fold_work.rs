// SPDX-License-Identifier: GPL-2.0-or-later

//! Count actual prefix and map work on the picture owner's thread.
use std::cell::Cell;

thread_local! {
    static WORK: Cell<Option<[usize; 2]>> = const { Cell::new(None) };
}

pub(super) fn record(prefixed: usize, written: usize) {
    if let Some([a, b]) = WORK.get() {
        WORK.set(Some([a + prefixed, b + written]));
    }
}

pub(crate) fn count<T>(fold: impl FnOnce() -> T) -> (T, [usize; 2]) {
    assert_eq!(WORK.replace(Some([0; 2])), None);
    let result = fold();
    (result, WORK.take().expect("fold observation is active"))
}
