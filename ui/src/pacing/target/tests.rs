// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

const REFRESH: Duration = Duration::from_nanos(10_266_000);
const FAST: Duration = Duration::from_millis(14);
const SLOW: Duration = Duration::from_millis(24);

/// Feeds one picture cost per 30.8 ms period and returns the target after each.
fn run(target: &mut Target, start: Instant, from: u32, costs: impl IntoIterator<Item = Duration>) -> Vec<u32> {
    costs
        .into_iter()
        .zip(from..)
        .map(|(cost, n)| {
            target.observe(start + REFRESH * 3 * n, cost, REFRESH);
            target.refreshes()
        })
        .collect()
}

#[test]
fn one_slow_picture_in_a_window_keeps_two_refreshes() {
    let mut target = Target::default();
    let costs = (0..320).map(|n| if n % WINDOW == 0 { SLOW } else { FAST });
    assert!(run(&mut target, Instant::now(), 0, costs).iter().all(|&refreshes| refreshes == 2));
}

#[test]
fn two_slow_pictures_in_a_window_move_to_three_and_stay_there() {
    let mut target = Target::default();
    let start = Instant::now();
    let costs = (0..640).map(|n| if n % 16 == 0 { SLOW } else { FAST });
    let seen = run(&mut target, start, 0, costs);
    let first = seen.iter().position(|&refreshes| refreshes == 3).unwrap();
    assert_eq!(first, WINDOW - 1);
    assert!(seen[first..].iter().all(|&refreshes| refreshes == 3), "a boundary view must not alternate");
}

#[test]
fn a_clean_window_returns_to_two_refreshes_after_the_hold() {
    let mut target = Target::default();
    let start = Instant::now();
    run(&mut target, start, 0, (0..64).map(|n| if n % 16 == 0 { SLOW } else { FAST }));
    assert_eq!(target.refreshes(), 3);
    let seen = run(&mut target, start, 64, std::iter::repeat_n(FAST, 200));
    let back = seen.iter().position(|&refreshes| refreshes == 2).unwrap();
    assert!(back >= WINDOW - 1, "returned with a slow picture still in the window");
    assert!(seen[back..].iter().all(|&refreshes| refreshes == 2));
}
