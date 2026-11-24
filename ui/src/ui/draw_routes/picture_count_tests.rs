// SPDX-License-Identifier: GPL-2.0-or-later
//! Reuse the actual HW RawBatch -> Ui::step route; no parallel rendering model.
use super::*;
fn value(text: &str, name: &str) -> u64 {
    text.split(" fresh.v1 ")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .find_map(|part| part.strip_prefix(&format!("{name}=")))
        .unwrap()
        .parse()
        .unwrap()
}
#[test]
fn actual_hw_compositions_count_first_use_once_and_retained_redraw_stays_empty() {
    let _scope = Scope::new();
    let mut r = Route::new(BatchKind::Raw, true, 4, 1e-6);
    r.ui.m.persist = 0;
    let counts = drawpipe::counters().timing.picture_counts();
    r.step(32);
    let first = counts.text();
    assert_eq!((value(&first, "fresh"), value(&first, "first_records")), (1, 32));
    let pixels = picture(&r.ui);
    r.ui.step(1.0 / 30.0);
    let redraw = counts.text();
    assert_eq!((value(&redraw, "fresh"), value(&redraw, "first_records")), (1, 32));
    assert_eq!(value(&redraw, "empty"), value(&first, "empty") + 1);
    assert_eq!(picture(&r.ui), pixels);
    r.step(32);
    let next = counts.text();
    assert_eq!((value(&next, "fresh"), value(&next, "first_records")), (2, 64));
    r.ui.m.run = RunState::Stop;
    r.ui.step(1.0 / 30.0);
    assert_eq!(value(&counts.text(), "fresh"), 2);
}
