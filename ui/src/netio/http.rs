// SPDX-License-Identifier: GPL-2.0-or-later
//! HTTP sockets share the instrument IO executor and bounded application admission.
mod input;
use super::{runtime, Runtime};
use crate::netport::{http::*, RemoteRequest};
use std::{
    io,
    net::SocketAddr,
    sync::{atomic::Ordering, Arc, Mutex},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinSet,
};

pub(crate) struct Service {
    addr: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
    stopped: std::sync::mpsc::Receiver<()>,
}
impl Service {
    pub(crate) fn start(
        port: crate::net6::Port,
        v6: bool,
        runtime: &Runtime,
        inbox: Arc<Inbox>,
        acl: Arc<Mutex<crate::netacl::Shared>>,
        limits: Limits,
    ) -> io::Result<Self> {
        let (ready, started) = std::sync::mpsc::sync_channel(1);
        let (finished, stopped) = std::sync::mpsc::sync_channel(1);
        let (stop, mut stopping) = oneshot::channel();
        runtime.spawn(async move {
            match runtime::listen(port, v6) {
                Ok(listener) => match listener.local_addr() {
                    Ok(addr) => {
                        let _ = ready.send(Ok(addr));
                        serve(listener, inbox, acl, limits, &mut stopping).await;
                    }
                    Err(e) => {
                        let _ = ready.send(Err(e));
                    }
                },
                Err(e) => {
                    let _ = ready.send(Err(e));
                }
            }
            let _ = finished.send(());
        });
        let addr = started.recv().map_err(io::Error::other)??;
        Ok(Self { addr, stop: Some(stop), stopped })
    }
    pub(crate) fn addr(&self) -> SocketAddr {
        self.addr
    }
}
impl Drop for Service {
    fn drop(&mut self) {
        self.stop.take();
        let _ = self.stopped.recv();
    }
}

async fn serve(
    listener: TcpListener,
    inbox: Arc<Inbox>,
    acl: Arc<Mutex<crate::netacl::Shared>>,
    limits: Limits,
    stopping: &mut oneshot::Receiver<()>,
) {
    let mut clients = JoinSet::new();
    loop {
        tokio::select! {
            _ = &mut *stopping => break,
            event = listener.accept() => {
                let Ok((mut socket, peer)) = event else { break };
                if inbox.active.load(Ordering::SeqCst) >= MAX_PENDING || clients.len() >= MAX_SESSIONS {
                    tokio::select! {
                        _ = &mut *stopping => break,
                        _ = input::answer(&mut socket, "503 Service Unavailable", "busy, try again") => {},
                    }
                    continue;
                }
                inbox.active.fetch_add(1, Ordering::SeqCst);
                let (inbox, acl) = (inbox.clone(), acl.clone());
                let active = Active(inbox.clone());
                clients.spawn(async move {
                    let _ = socket.set_nodelay(true);
                    let _ = conversation(&mut socket, peer, inbox, acl, limits, active).await;
                });
            }
            Some(_) = clients.join_next(), if !clients.is_empty() => {}
        }
    }
    clients.abort_all();
    while clients.join_next().await.is_some() {}
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
