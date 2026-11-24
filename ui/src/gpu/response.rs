// SPDX-License-Identifier: GPL-2.0-or-later

use super::{Times, K_DONE, MAGIC};
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
        self.decode(times).map(Some)
    }

    pub(super) fn decode(&self, times: &mut Times) -> Result<Duration, String> {
        let (words, _) = self.bytes.as_chunks::<4>();
        let word = |i: usize| u32::from_ne_bytes(words[i]);
        if word(0) != MAGIC || word(1) != K_DONE {
            return Err("bad answer".into());
        }
        if word(2) != 0 {
            return Err(format!("helper error {}", word(2)));
        }
        [times.upload, times.draw, times.read] = [word(3), word(4), word(5)].map(f32::from_bits);
        let [_, _, _, time] = self.bytes.as_chunks::<8>().0 else { unreachable!("32-byte response") };
        Ok(Duration::from_nanos(u64::from_ne_bytes(*time)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragmented_reply_stays_pending_and_keeps_helper_timestamp() {
        let bytes = [MAGIC, K_DONE, 0, 1.0f32.to_bits(), 2.0f32.to_bits(), 0].map(u32::to_ne_bytes).concat();
        let time = 8_765_432_100u64;
        let mut rest = [&bytes[..], &time.to_ne_bytes()].concat();
        let mut response = Response::default();
        let mut times = Times::default();
        let mut calls = 0;
        assert_eq!(
            response.poll(&mut times, false, |buf| {
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
            response.poll(&mut times, false, |buf| {
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
        assert!(response.poll(&mut Times::default(), false, |buf| io::Read::read(&mut input, buf)).is_err());
    }

    #[test]
    fn a_partial_reply_times_out_without_reading_into_the_next_ticket() {
        let mut response = Response::default();
        let mut times = Times::default();
        let mut read = |_buf: &mut [u8]| Err(io::ErrorKind::WouldBlock.into());
        assert_eq!(response.poll(&mut times, false, &mut read), Ok(None));
        assert_eq!(response.poll(&mut times, true, read), Err("helper reply timed out".into()));
    }
}
