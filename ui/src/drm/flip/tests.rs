// SPDX-License-Identifier: GPL-2.0-or-later

use super::*;

fn tagged(kind: u32, cookie: u64, payload: [u8; 16]) -> Vec<u8> {
    [kind.to_ne_bytes().as_slice(), &32u32.to_ne_bytes(), &cookie.to_ne_bytes(), &payload].concat()
}

#[test]
fn a_read_can_complete_the_panel_flip_and_vblank_independently() {
    let now = Instant::now();
    let mut state = FlipState { flip: Some((5, now)), vblank: Some((6, now)), ..Default::default() };
    let flip =
        |cookie, seq| tagged(2, cookie, [100u32, 1_000, seq, 0].map(u32::to_ne_bytes).concat().try_into().unwrap());
    let stamp = 100_001_000_250u64;
    let mut stream = flip(4, 100);
    stream.extend(tagged(3, 6, [stamp, u64::from(u32::MAX) + 102].map(u64::to_ne_bytes).concat().try_into().unwrap()));
    stream.extend(flip(5, 101));

    let events = state.read(&stream);
    let blank = events.vblank.unwrap();
    let shown = events.flip.unwrap();
    assert_eq!((blank.sequence, shown.sequence), (101, 101));
    assert_eq!(blank.monotonic, Duration::from_nanos(stamp));
    assert_eq!(shown.monotonic, Duration::from_micros(100_001_000));
    assert!(state.flip.is_none() && state.vblank.is_none());
    let repeat = state.read(&stream);
    assert!(repeat.flip.is_none() && repeat.vblank.is_none());
}

#[test]
fn hdmi_mirror_keeps_scanout_while_busy_and_can_fall_back_to_modeset() {
    assert!(quiet_flip(Ok(()), || panic!("a queued HDMI flip needs no modeset")));
    assert!(!quiet_flip(Err(io::Error::from_raw_os_error(libc::EBUSY)), || panic!(
        "the old buffer is still on scanout"
    )));
    assert!(quiet_flip(Err(io::Error::from_raw_os_error(libc::EINVAL)), || Ok(())));
    assert!(!quiet_flip(Err(io::Error::from_raw_os_error(libc::EINVAL)), || Err(io::ErrorKind::NotConnected.into())));
}

fn display(flips: FlipState) -> Display {
    Display {
        fd: -1,
        width: 0,
        height: 0,
        conn_id: 0,
        crtc_id: 0,
        mode: ModeInfo {
            clock: 0,
            hdisplay: 0,
            hsync_start: 0,
            hsync_end: 0,
            htotal: 0,
            hskew: 0,
            vdisplay: 0,
            vsync_start: 0,
            vsync_end: 0,
            vtotal: 0,
            vscan: 0,
            vrefresh: 60,
            flags: 0,
            typ: 0,
            name: [0; 32],
        },
        saved: None,
        bufs: Vec::new(),
        front: 0,
        prev: 2,
        flips,
        fit: None,
    }
}

#[test]
fn a_stale_cookie_gives_the_next_busy_request_a_full_grace_period() {
    use FlipRequest::{Busy, Modeset, Queued};
    let start = Instant::now();
    let busy = Some(libc::EBUSY);
    for prior_flip in [false, true] {
        let mut display =
            display(FlipState { flip: prior_flip.then_some((9, start)), serial: 9, ..Default::default() });
        let mut turn = |offset, script: &[Option<i32>], want| {
            let mut results = script.iter();
            let got = display.request_flip_with(
                1,
                start + offset,
                true,
                |_, _| results.next().unwrap().map_or(Ok(()), |errno| Err(io::Error::from_raw_os_error(errno))),
                |_| panic!("SETCRTC succeeds"),
            );
            assert!(results.next().is_none());
            assert_eq!(got, want);
            assert_eq!(display.front, usize::from(want != Busy));
        };
        let grace_start = if prior_flip { FLIP_TIMEOUT } else { Duration::ZERO };
        if prior_flip {
            turn(Duration::ZERO, &[], Busy);
        }
        turn(grace_start, &[busy], Busy);
        turn(grace_start + FLIP_TIMEOUT - Duration::from_millis(1), &[busy], Busy);
        turn(grace_start + FLIP_TIMEOUT, &[busy, None], Modeset);
        turn(grace_start + FLIP_TIMEOUT, &[None], Queued);
        let event = tagged(2, 9, [0; 16]);
        assert!(display.flips.read(&event).flip.is_none());
    }
}

#[test]
fn failed_modesets_report_once_until_scanout_recovers() {
    let mut display = display(FlipState::default());
    let now = Instant::now();
    let mut reports = 0;
    for (step, fails, expected_reports) in [(0, true, 1), (1, true, 1), (2, false, 1), (3, true, 2), (4, true, 2)] {
        let mut requests = Vec::new();
        let got = display.request_flip_with(
            1,
            now,
            true,
            |_, request| {
                requests.push(request);
                match request {
                    Some(_) => Err(io::Error::from_raw_os_error(libc::EINVAL)),
                    None if fails => Err(io::Error::from_raw_os_error(libc::EIO)),
                    None => Ok(()),
                }
            },
            |error| {
                assert_eq!(error.raw_os_error(), Some(libc::EIO));
                reports += 1;
            },
        );
        let expected = if step == 0 { vec![Some(1), None] } else { vec![None] };
        assert_eq!(requests, expected);
        assert_eq!(reports, expected_reports);
        assert_eq!(got, if fails { FlipRequest::Failed } else { FlipRequest::Modeset });
    }
}

#[test]
fn immediate_modeset_waits_until_the_target_refresh() {
    let mut display = display(FlipState::default());
    let start = Instant::now();
    let flip = display.request_flip_with(
        1,
        start,
        false,
        |_, cookie| {
            assert!(cookie.is_some());
            Err(io::Error::from_raw_os_error(libc::EINVAL))
        },
        |_| panic!("unexpected failure"),
    );
    assert_eq!(flip, FlipRequest::AwaitTarget);
    assert_eq!(display.front, 0);
    let flip = display.request_flip_with(
        1,
        start + Duration::from_millis(10),
        true,
        |_, cookie| {
            assert!(cookie.is_none());
            Ok(())
        },
        |_| panic!("unexpected failure"),
    );
    assert_eq!(flip, FlipRequest::Modeset);
    assert_eq!(display.front, 1);
}

#[cfg(target_os = "linux")]
impl Display {
    pub(crate) fn scanout_fixture(fd: i32) -> Self {
        let mut display = display(FlipState { emulate_scanout: true, ..Default::default() });
        display.fd = fd;
        // A zero-sized output accepts flips without requiring mapped buffers.
        display.bufs = (0..3).map(|fb| Buf { handle: 0, fb, pitch: 0, size: 0, map: std::ptr::null_mut() }).collect();
        display
    }
}
