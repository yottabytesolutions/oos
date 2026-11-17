// SPDX-License-Identifier: GPL-2.0-or-later
//! Send deadlines use IO time; deterministic fixtures supply their own clock and timer futures.
use super::timer::SendTimer;
use crate::netport::Reply;
use std::{future::Future, io, time::Duration};
use tokio::io::AsyncWrite;

pub(super) async fn send<F: Future<Output = ()>>(
    writer: &mut (impl AsyncWrite + Unpin),
    reply: Reply,
    mut now: impl FnMut() -> Duration,
    mut wait: impl FnMut(Duration) -> F,
) -> io::Result<()> {
    let mut timer = SendTimer::new(now());
    for bytes in reply.output().chunks() {
        let mut position = 0;
        while position < bytes.len() {
            if timer.expired(now()) {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let n = tokio::select! {
                biased;
                _ = wait(timer.deadline()) => return Err(io::ErrorKind::TimedOut.into()),
                n = std::future::poll_fn(|cx| std::pin::Pin::new(&mut *writer).poll_write(cx, &bytes[position..])) => n?,
            };
            if n == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            position += n;
            timer.progress(now());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
