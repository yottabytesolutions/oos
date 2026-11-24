// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Times, K_DONE, K_READBACK, MAGIC};
use std::{io, time::Duration};

pub(super) const SIZE: usize = 32;

#[derive(Default)]
pub(super) struct Response {
    bytes: [u8; SIZE],
    read: usize,
}

impl Response {
    pub(super) fn remaining(&mut self) -> &mut [u8] {
        &mut self.bytes[self.read..]
    }

    pub(super) fn poll(
        &mut self,
        kind: u32,
        times: &mut Times,
        expired: bool,
        mut read: impl FnMut(&mut [u8]) -> io::Result<usize>,
    ) -> Result<Option<Duration>, String> {
        while self.read < SIZE {
            match read(self.remaining()) {
                Ok(0) => return Err("helper disconnected".into()),
                Ok(n) => self.read += n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock && !expired => return Ok(None),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return Err("helper reply timed out".into()),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        self.decode(kind, times).map(Some)
    }

    pub(super) fn decode(&self, kind: u32, times: &mut Times) -> Result<Duration, String> {
        let (words, _) = self.bytes.as_chunks::<4>();
        let word = |i: usize| u32::from_ne_bytes(words[i]);
        if word(0) != MAGIC || word(1) != kind {
            return Err("bad answer".into());
        }
        if word(2) != 0 {
            return Err(format!("helper error {}", word(2)));
        }
        if kind == K_READBACK {
            times.read = f32::from_bits(word(5));
        } else {
            debug_assert_eq!(kind, K_DONE);
            [times.upload, times.draw] = [word(3), word(4)].map(f32::from_bits);
        }
        let [_, _, _, time] = self.bytes.as_chunks::<8>().0 else { unreachable!("32-byte response") };
        Ok(Duration::from_nanos(u64::from_ne_bytes(*time)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_completion_does_not_wait_for_the_requested_pixels() {
        let mut times = Times::default();
        let render = [MAGIC, K_DONE, 0, 2.0f32.to_bits(), 10.0f32.to_bits(), 0, 12_000_000, 0];
        let pixels = [MAGIC, K_READBACK, 0, 0, 0, 11.0f32.to_bits(), 23_000_000, 0];
        let bytes = [render, pixels].map(|reply| reply.map(u32::to_ne_bytes).concat());
        let mut first = bytes[0].as_slice();
        let mut reply = Response::default();
        assert_eq!(
            reply.poll(K_DONE, &mut times, false, |buf| io::Read::read(&mut first, buf)),
            Ok(Some(Duration::from_millis(12)))
        );
        assert_eq!((times.upload, times.draw, times.read), (2.0, 10.0, 0.0));
        let mut reply = Response::default();
        assert_eq!(reply.poll(K_READBACK, &mut times, false, |_| Err(io::ErrorKind::WouldBlock.into())), Ok(None));
        let mut second = bytes[1].as_slice();
        assert_eq!(
            reply.poll(K_READBACK, &mut times, false, |buf| io::Read::read(&mut second, buf)),
            Ok(Some(Duration::from_millis(23)))
        );
        assert_eq!((times.upload, times.draw, times.read), (2.0, 10.0, 11.0));
        assert!(reply.decode(K_DONE, &mut times).is_err());
        let mut render = bytes[0].as_slice();
        let mut reply = Response::default();
        assert!(reply.poll(K_DONE, &mut times, false, |buf| io::Read::read(&mut render, buf)).unwrap().is_some());
        assert_eq!(times.read, 11.0);
    }

    #[test]
    fn fragmented_reply_stays_pending_and_keeps_helper_timestamp() {
        let bytes = [MAGIC, K_DONE, 0, 1.0f32.to_bits(), 2.0f32.to_bits(), 0].map(u32::to_ne_bytes).concat();
        let time = 8_765_432_100u64;
        let mut rest = [&bytes[..], &time.to_ne_bytes()].concat();
        let mut response = Response::default();
        let mut times = Times::default();
        let mut calls = 0;
        assert_eq!(
            response.poll(K_DONE, &mut times, false, |buf| {
                calls += 1;
                if calls > 1 {
                    return Err(io::ErrorKind::WouldBlock.into());
                }
                buf[..7].copy_from_slice(&rest[..7]);
                rest.drain(..7);
                Ok(7)
            }),
            Ok(None)
        );
        assert_eq!(
            response.poll(K_DONE, &mut times, false, |buf| {
                buf.copy_from_slice(&rest);
                Ok(rest.len())
            }),
            Ok(Some(Duration::from_nanos(time)))
        );
        assert_eq!((times.upload, times.draw), (1.0, 2.0));
    }

    #[test]
    fn closed_helper_with_an_incomplete_reply_fails() {
        let mut response = Response::default();
        let mut input = [0; 7].as_slice();
        assert!(response.poll(K_DONE, &mut Times::default(), false, |buf| io::Read::read(&mut input, buf)).is_err());
    }

    #[test]
    fn a_partial_reply_times_out_without_reading_into_the_next_ticket() {
        let mut response = Response::default();
        let mut times = Times::default();
        let mut read = |_buf: &mut [u8]| Err(io::ErrorKind::WouldBlock.into());
        assert_eq!(response.poll(K_DONE, &mut times, false, &mut read), Ok(None));
        assert_eq!(response.poll(K_DONE, &mut times, true, read), Err("helper reply timed out".into()));
    }
}
