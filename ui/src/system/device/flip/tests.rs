// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;
use crate::drm::Display;
use crate::pacing::{self, presentation::FLIP_TIMEOUT, ClockMode, FlipDivisor, FrameClock};
use std::{io::Write, os::fd::AsRawFd, os::unix::net::UnixStream};

#[test]
fn queue_panel_accounts_for_a_delivered_flip_before_timing_out() {
    let (rx, mut tx) = UnixStream::pair().unwrap();
    let mut out = Outputs::new(Display::scanout_fixture(rx.as_raw_fd()), false);
    let now = Instant::now();
    let clock = FrameClock::new(ClockMode::Flip, FlipDivisor::Three, now, Duration::from_millis(10));
    out.panel.stage(1, clock.seal());
    out.service_panel(now, None);
    let mut before = String::new();
    pacing::append_presentation(&mut before);

    let event = [2u32, 32, 1, 0, 0, 0, 3, 0].map(u32::to_ne_bytes);
    tx.write_all(event.as_flattened()).unwrap();
    out.queue_panel(0, clock.seal(), now + FLIP_TIMEOUT);

    let mut after = String::new();
    pacing::append_presentation(&mut after);
    let count = |text: &str, field: &str| {
        text.split_ascii_whitespace().find_map(|part| part.strip_prefix(field)).unwrap().parse::<u64>().unwrap()
    };
    assert_eq!(count(&after, "flip.shown="), count(&before, "flip.shown=") + 1);
    assert_eq!(count(&after, "flip.timeout="), count(&before, "flip.timeout="));
}
