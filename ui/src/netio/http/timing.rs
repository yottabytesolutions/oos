// SPDX-License-Identifier: GPL-2.0-or-later
//! Observe transport progress while hyper owns the connection and wire format.
use super::super::timer::SendTimer;
use super::*;
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::watch,
    time::Instant,
};

#[derive(Clone)]
pub(super) struct Progress(watch::Sender<State>);
#[derive(Clone, Copy)]
struct State {
    started: Instant,
    read: Instant,
    reading: bool,
    send: Option<SendTimer>,
}
impl Progress {
    pub(super) fn new() -> Self {
        let now = Instant::now();
        Self(watch::channel(State { started: now, read: now, reading: true, send: None }).0)
    }
    pub(super) fn finish_read(&self) {
        self.0.send_modify(|s| s.reading = false);
    }
    pub(super) fn begin_send(&self) {
        self.0.send_modify(|s| {
            s.reading = false;
            s.send = Some(SendTimer::new(s.started.elapsed()));
        });
    }
    fn written(&self, result: &Poll<io::Result<usize>>) {
        if matches!(result, Poll::Ready(Ok(n)) if *n > 0) {
            self.0.send_modify(|s| {
                if let Some(timer) = &mut s.send {
                    timer.progress(s.started.elapsed());
                }
            });
        }
    }
    pub(super) async fn expired(&self, limits: Limits) -> Option<&'static str> {
        let mut changes = self.0.subscribe();
        loop {
            let state = *changes.borrow_and_update();
            let deadline = if state.reading {
                Some((state.started + limits.deadline).min(state.read + limits.idle))
            } else {
                state.send.map(|timer| state.started + timer.deadline())
            };
            if deadline.is_some_and(|at| Instant::now() >= at) {
                return state.reading.then_some(if Instant::now() >= state.started + limits.deadline {
                    "request took too long"
                } else {
                    "request timed out"
                });
            }
            tokio::select! {
                biased;
                _ = changes.changed() => {},
                _ = async { if let Some(at) = deadline { tokio::time::sleep_until(at).await } else { std::future::pending().await } } => {},
            }
        }
    }
}

pub(super) struct Io<T> {
    pub(super) socket: T,
    pub(super) progress: Progress,
}
impl<T: AsyncRead + Unpin> AsyncRead for Io<T> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let result = Pin::new(&mut self.socket).poll_read(cx, buf);
        if matches!(result, Poll::Ready(Ok(()))) && buf.filled().len() > before {
            self.progress.0.send_modify(|s| s.read = Instant::now());
        }
        result
    }
}
impl<T: AsyncWrite + Unpin> AsyncWrite for Io<T> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.socket).poll_write(cx, bytes);
        self.progress.written(&result);
        result
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bytes: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.socket).poll_write_vectored(cx, bytes);
        self.progress.written(&result);
        result
    }
    fn is_write_vectored(&self) -> bool {
        self.socket.is_write_vectored()
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.socket).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.socket).poll_shutdown(cx)
    }
}
