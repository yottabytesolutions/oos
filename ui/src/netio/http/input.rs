// SPDX-License-Identifier: GPL-2.0-or-later
//! Hyper supplies framing; policy and admission run before the first body poll.
use super::*;
use http_body_util::{BodyExt, Full};
use hyper::{body::Bytes, Request, Response};
pub(super) type WireResponse = Response<Full<Bytes>>;

pub(super) struct Input {
    pub(super) inbox: Arc<Inbox>,
    pub(super) acl: Arc<Mutex<crate::netacl::Shared>>,
    pub(super) peer: SocketAddr,
    pub(super) limits: Limits,
    pub(super) active: Mutex<Option<Active>>,
}
impl Input {
    pub(super) async fn handle(
        &self,
        request: Request<hyper::body::Incoming>,
        progress: &timing::Progress,
    ) -> io::Result<WireResponse> {
        let head = parse_head(&request);
        let policy = oos_hal::sync::lock(&self.acl).clone();
        let peer_ok = super::super::runtime::permits(policy, Ok(self.peer)).await;
        if let Some((status, message)) = refuse_early(&head, request.headers(), peer_ok) {
            return Ok(answer(status, message));
        }
        let large = if head.len.is_some_and(|n| n > LARGE_BODY) {
            let Some(permit) = self.inbox.large.admit_large_body(head.len.unwrap_or(0)) else {
                return Ok(answer("503 Service Unavailable", "busy, try again"));
            };
            Some(permit)
        } else {
            None
        };
        let (parts, mut incoming) = request.into_parts();
        let mut bytes = Vec::with_capacity(head.len.unwrap_or(0));
        while let Some(frame) = incoming.frame().await {
            let frame = frame.map_err(io::Error::other)?;
            if let Ok(data) = frame.into_data() {
                bytes.extend_from_slice(&data);
            }
        }
        progress.finish_read();
        let body = if head.post && head.route == "/update" {
            match Upload::new(bytes) {
                Ok(upload) => Body::Upload(Some(upload)),
                Err(_) => return Ok(answer("409 Conflict", "REFUSED: no thread for the check, nothing installed")),
            }
        } else {
            Body::Bytes(bytes)
        };
        let envelope = self.inbox.envelope(Request::from_parts(parts, ()), body, self.peer);
        let RemoteRequest::Http(req) = envelope.request() else {
            return Err(io::Error::other("HTTP request expected"));
        };
        let reply = req.reply.clone();
        let _connection = Connection(reply.clone());
        let id = envelope.id();
        let _credit = envelope.credit(self.inbox.ports.clone());
        if !reply.static_content(&req.head) {
            let size = req.body.capacity() + MAX_HEAD;
            if self.inbox.ports.requests().submit(id, envelope, size).is_err()
                || !wait_taken(&reply, tokio::time::sleep(self.limits.take), || self.inbox.ports.requests().cancel(id))
                    .await
            {
                return Ok(answer("503 Service Unavailable", "busy, try again"));
            }
        }
        drop((oos_hal::sync::lock(&self.active).take(), large));
        let Some(mut response) = reply.receive().await else { return Err(io::Error::other("HTTP response cancelled")) };
        let builder = std::mem::replace(&mut response.head, Response::builder());
        builder.body(Full::new(Bytes::from_owner(Output { response, _credit, _connection }))).map_err(io::Error::other)
    }
}

struct Output {
    response: crate::netport::http::Response,
    _credit: crate::netport::Credit,
    _connection: Connection,
}
impl AsRef<[u8]> for Output {
    fn as_ref(&self) -> &[u8] {
        self.response.as_ref()
    }
}

pub(super) fn answer(status: &str, message: &str) -> WireResponse {
    response(status)
        .header("content-type", "text/plain")
        .body(Full::new(Bytes::copy_from_slice(message.as_bytes())))
        .expect("HTTP refusal status and headers are constants")
}

/// Use hyper's writer for refusals made before a request head exists, too.
pub(super) async fn refuse(
    socket: &mut (impl tokio::io::AsyncWrite + Unpin),
    status: &str,
    message: &str,
) -> io::Result<()> {
    let io = tokio::io::join(&b"GET / HTTP/1.1\r\n\r\n"[..], socket);
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        hyper::server::conn::http1::Builder::new()
            .keep_alive(false)
            .half_close(true)
            .auto_date_header(false)
            .title_case_headers(true)
            .serve_connection(
                hyper_util::rt::TokioIo::new(io),
                hyper::service::service_fn(|_| async { Ok::<_, io::Error>(answer(status, message)) }),
            ),
    )
    .await?
    .map_err(io::Error::other)
}
