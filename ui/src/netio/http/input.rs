// SPDX-License-Identifier: GPL-2.0-or-later
//! HTTP framing samples elapsed time explicitly; stalled reads never block the executor.
use crate::netport::http::*;
use std::{future::Future, io, net::SocketAddr, sync::Mutex, time::Duration};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
type Refusal = (&'static str, &'static str);
type Large<'a> = Option<crate::netport::UploadPermit<'a>>;
pub(super) struct Input<'a> {
    pub(super) inbox: &'a Inbox,
    pub(super) acl: &'a Mutex<crate::netacl::Shared>,
    pub(super) peer: SocketAddr,
    pub(super) limits: Limits,
}
impl<'a> Input<'a> {
    pub(super) async fn read<F: Future<Output = ()>>(
        &self,
        socket: &mut (impl AsyncRead + AsyncWrite + Unpin),
        mut now: impl FnMut() -> Duration,
        mut wait: impl FnMut(Duration) -> F,
    ) -> io::Result<Option<(String, Body, Large<'a>)>> {
        let mut bytes = Vec::new();
        let result = async {
            let end = loop {
                if let Some(p) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break p + 4;
                }
                if bytes.len() > MAX_HEAD {
                    return Err(("431 Request Header Fields Too Large", "request head too long"));
                }
                read_more(socket, &mut bytes, MAX_HEAD + 65536, self.limits, &mut now, &mut wait).await?;
            };
            let text = String::from_utf8_lossy(&bytes[..end]).into_owned();
            let head = parse_head(&text);
            let policy = oos_hal::sync::lock(self.acl).clone();
            let peer_ok = super::super::runtime::permits(policy, Ok(self.peer)).await;
            if let Some(refusal) = refuse_early(&head, &text, peer_ok) {
                return Err(refusal);
            }
            let slot = if head.len.is_some_and(|n| n > LARGE_BODY) {
                Some(
                    self.inbox
                        .large
                        .admit_large_body(head.len.unwrap_or(0))
                        .ok_or(("503 Service Unavailable", "busy, try again"))?,
                )
            } else {
                None
            };
            let body = self.body(socket, bytes, end, &head, &mut now, &mut wait).await?;
            Ok((text, body, slot))
        }
        .await;
        match result {
            Ok(request) => Ok(Some(request)),
            Err(("", "")) => Ok(None),
            Err((status, message)) => {
                answer(socket, status, message).await?;
                drain(socket).await;
                Ok(None)
            }
        }
    }
    async fn body<F: Future<Output = ()>>(
        &self,
        socket: &mut (impl AsyncRead + AsyncWrite + Unpin),
        mut bytes: Vec<u8>,
        end: usize,
        head: &Head,
        now: &mut impl FnMut() -> Duration,
        wait: &mut impl FnMut(Duration) -> F,
    ) -> Result<Body, Refusal> {
        let need = end + head.len.unwrap_or(0);
        bytes.reserve_exact(need.saturating_sub(bytes.len()));
        if head.expect_continue && bytes.len() < need {
            match tokio::time::timeout(Duration::from_secs(1), socket.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")).await
            {
                Ok(Ok(())) => {}
                _ => return Err(("", "")),
            }
        }
        while bytes.len() < need {
            read_more(socket, &mut bytes, need, self.limits, now, wait).await?;
        }
        bytes.truncate(need);
        let body = bytes.split_off(end);
        if head.post && head.route == "/update" {
            Ok(Body::Upload(Some(
                Upload::new(body)
                    .map_err(|_| ("409 Conflict", "REFUSED: no thread for the check, nothing installed"))?,
            )))
        } else {
            Ok(Body::Bytes(body))
        }
    }
}

async fn read_more<F: Future<Output = ()>>(
    socket: &mut (impl AsyncRead + Unpin),
    bytes: &mut Vec<u8>,
    maximum: usize,
    limits: Limits,
    now: &mut impl FnMut() -> Duration,
    wait: &mut impl FnMut(Duration) -> F,
) -> Result<(), Refusal> {
    let elapsed = now();
    if elapsed >= limits.deadline {
        return Err(("408 Request Timeout", "request took too long"));
    }
    let deadline = (elapsed + limits.idle).min(limits.deadline);
    let mut buffer = [0; 65536];
    let length = buffer.len().min(maximum.saturating_sub(bytes.len()));
    let result = tokio::select! {
        biased;
        _ = wait(deadline) => return Err(("408 Request Timeout", "request timed out")),
        result = socket.read(&mut buffer[..length]) => result,
    };
    match result {
        Ok(0) | Err(_) => Err(("", "")),
        Ok(n) => {
            bytes.extend_from_slice(&buffer[..n]);
            Ok(())
        }
    }
}

pub(super) async fn answer(socket: &mut (impl AsyncWrite + Unpin), status: &str, message: &str) -> io::Result<()> {
    let head = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        message.len(),
        message
    );
    tokio::time::timeout(Duration::from_secs(1), socket.write_all(head.as_bytes())).await??;
    socket.shutdown().await
}
async fn drain(socket: &mut (impl AsyncRead + Unpin)) {
    let mut bytes = [0; 16384];
    let _ = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            match tokio::time::timeout(Duration::from_millis(200), socket.read(&mut bytes)).await {
                Ok(Ok(n)) if n != 0 => {}
                _ => break,
            }
        }
    })
    .await;
}
