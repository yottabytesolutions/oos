// SPDX-License-Identifier: GPL-2.0-or-later
//! HTTP sockets share the instrument IO executor and bounded application admission.
mod input;
use super::port::{http::*, RemoteRequest};
use super::{runtime, Runtime};
use std::{
    io,
    net::SocketAddr,
    sync::{atomic::Ordering, Arc, Mutex},
};
use tokio::net::TcpStream;

pub(crate) struct Service(super::executor::Task<SocketAddr>);
impl Service {
    pub(crate) fn start(
        port: crate::net6::Port,
        v6: bool,
        runtime: &Runtime,
        inbox: Arc<Inbox>,
        acl: Arc<Mutex<crate::netacl::Shared>>,
        limits: Limits,
    ) -> io::Result<Self> {
        runtime
            .start(move |stop| {
                let listener = runtime::listen(port, v6)?;
                Ok((listener.local_addr()?, async move {
                    super::listener::serve(
                        MAX_SESSIONS,
                        stop,
                        || listener.accept(),
                        |(mut socket, peer), capacity| {
                            let (inbox, acl) = (inbox.clone(), acl.clone());
                            async move {
                                if matches!(capacity, super::listener::Capacity::Full)
                                    || inbox.active.load(Ordering::SeqCst) >= MAX_PENDING
                                {
                                    let _ =
                                        input::answer(&mut socket, "503 Service Unavailable", "busy, try again").await;
                                    return None;
                                }
                                inbox.active.fetch_add(1, Ordering::SeqCst);
                                let active = Active(inbox.clone());
                                Some(async move {
                                    let _ = socket.set_nodelay(true);
                                    let _ = conversation(&mut socket, peer, inbox, acl, limits, active).await;
                                })
                            }
                        },
                    )
                    .await;
                }))
            })
            .map(Self)
    }
    pub(crate) fn addr(&self) -> SocketAddr {
        self.0.local
    }
}

struct Active(Arc<Inbox>);
impl Drop for Active {
    fn drop(&mut self) {
        self.0.active.fetch_sub(1, Ordering::SeqCst);
    }
}
struct Connection(ReplyHandle);
impl Drop for Connection {
    fn drop(&mut self) {
        self.0.close();
    }
}
async fn conversation(
    socket: &mut TcpStream,
    peer: SocketAddr,
    inbox: Arc<Inbox>,
    acl: Arc<Mutex<crate::netacl::Shared>>,
    limits: Limits,
    active: Active,
) -> io::Result<()> {
    let start = tokio::time::Instant::now();
    let input = input::Input { inbox: &inbox, acl: &acl, peer, limits };
    let Some((text, body, large)) =
        input.read(socket, || start.elapsed(), |deadline| tokio::time::sleep_until(start + deadline)).await?
    else {
        return Ok(());
    };
    let bytes = text.capacity() + body.capacity();
    let envelope = inbox.envelope(text, body, peer);
    let RemoteRequest::Http(req) = envelope.request() else { return Ok(()) };
    let reply = req.reply.clone();
    let _connection = Connection(reply.clone());
    let id = envelope.id();
    let _credit = envelope.credit(inbox.ports.clone());
    if !reply.static_content(&req.head) {
        if inbox.ports.requests().submit(id, envelope, bytes).is_err() {
            return input::answer(socket, "503 Service Unavailable", "busy, try again").await;
        }
        let taken = tokio::select! {
            taken = wait_taken(&reply, tokio::time::sleep(limits.take), || inbox.ports.requests().cancel(id)) => taken,
            _ = read_failed(socket) => return Ok(()),
        };
        if !taken {
            return input::answer(socket, "503 Service Unavailable", "busy, try again").await;
        }
    }
    drop((active, large));
    let response = tokio::select! {
        response = reply.receive() => response,
        _ = read_failed(socket) => return Ok(()),
    };
    let Some(response) = response else { return Ok(()) };
    let start = tokio::time::Instant::now();
    super::output::send(
        socket,
        response.reply,
        || start.elapsed(),
        |deadline| tokio::time::sleep_until(start + deadline),
    )
    .await
}

#[cfg(test)]
pub(crate) mod tests;

async fn read_failed(socket: &TcpStream) -> io::Result<()> {
    super::session::disconnected(socket).await?;
    // Read EOF leaves the response direction open after a complete request.
    std::future::pending().await
}

async fn wait_taken(
    reply: &ReplyHandle,
    timeout: impl std::future::Future<Output = ()>,
    cancel: impl FnOnce() -> bool,
) -> bool {
    tokio::select! {
        biased;
        _ = reply.taken() => true,
        _ = timeout => !cancel(),
    }
}
