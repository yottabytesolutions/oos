// SPDX-License-Identifier: GPL-2.0-or-later
//! HTTP sockets share the instrument IO executor and bounded application admission.
mod input;
mod timing;
use super::{runtime, Runtime};
use crate::netport::{http::*, RemoteRequest};
use std::{
    io,
    net::SocketAddr,
    sync::{atomic::Ordering, Arc, Mutex},
};
use tokio::{net::TcpListener, sync::oneshot, task::JoinSet};

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
                        _ = input::refuse(&mut socket, "503 Service Unavailable", "busy, try again") => {},
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
    socket: &mut (impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin),
    peer: SocketAddr,
    inbox: Arc<Inbox>,
    acl: Arc<Mutex<crate::netacl::Shared>>,
    limits: Limits,
    active: Active,
) -> io::Result<()> {
    let progress = timing::Progress::new();
    let input = Arc::new(input::Input { inbox, acl, peer, limits, active: Mutex::new(Some(active)) });
    let timed_out_read = {
        let io = timing::Io { socket: &mut *socket, progress: progress.clone() };
        let service = hyper::service::service_fn(|request| {
            let (input, progress) = (input.clone(), progress.clone());
            async move {
                let response = input.handle(request, &progress).await;
                progress.begin_send();
                response
            }
        });
        let mut builder = hyper::server::conn::http1::Builder::new();
        builder
            .keep_alive(false)
            .half_close(true)
            .auto_date_header(false)
            .title_case_headers(true)
            // Queued buffers retain payload owners and admission until socket output completes.
            .writev(true)
            .max_headers(MAX_HEAD / 4)
            .max_header_size(MAX_HEAD)
            .max_buf_size(MAX_HEAD);
        let connection = builder.serve_connection(hyper_util::rt::TokioIo::new(io), service);
        tokio::select! {
            biased;
            reading = progress.expired(limits) => reading,
            result = connection => return result.map_err(io::Error::other),
        }
    };
    if let Some(message) = timed_out_read {
        input::refuse(socket, "408 Request Timeout", message).await
    } else {
        Err(io::ErrorKind::TimedOut.into())
    }
}

#[cfg(test)]
pub(crate) mod tests;
#[cfg(test)]
mod transport_tests;

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
