// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::netport::{Reply, MAX_CLIENTS};
use std::collections::HashMap;
use tokio::{
    net::{TcpListener, UnixDatagram},
    sync::{mpsc, watch},
    task::JoinSet,
};

pub(super) async fn run(
    port: crate::net6::Port,
    v6: bool,
    ports: Arc<Ports>,
    changed: watch::Receiver<Option<crate::netacl::Shared>>,
    ready: std::sync::mpsc::SyncSender<io::Result<BoundPorts>>,
    protocol: Protocol,
) {
    let result = async {
        let listener = listen(port, v6)?;
        let mapper = if matches!(protocol, Protocol::Rpc(_)) {
            Some(Arc::new(super::rpc::Mapper::open(port, v6)?))
        } else {
            None
        };
        let bound = (listener.local_addr()?.port(), mapper.as_ref().map(|m| m.ports()).transpose()?);
        let wake = UnixDatagram::from_std(ports.io_wake().receiver()?)?;
        let _ = ready.send(Ok(bound));
        serve(listener, mapper, wake, ports.clone(), changed, protocol).await;
        Ok(())
    }
    .await;
    if let Err(e) = result {
        let _ = ready.send(Err(e));
    }
    ports.close();
}

pub(super) fn listen(port: crate::net6::Port, v6: bool) -> io::Result<TcpListener> {
    let listener = crate::net6::tcp(port, v6).ok_or_else(|| io::Error::other("listener unavailable"))?;
    listener.set_nonblocking(true)?;
    TcpListener::from_std(listener)
}

async fn serve(
    listener: TcpListener,
    mapper: Option<Arc<super::rpc::Mapper>>,
    wake: UnixDatagram,
    ports: Arc<Ports>,
    mut changed: watch::Receiver<Option<crate::netacl::Shared>>,
    protocol: Protocol,
) {
    let mut clients = JoinSet::new();
    let mut sessions: HashMap<u64, (mpsc::Sender<Reply>, Option<watch::Sender<crate::netacl::Shared>>)> =
        HashMap::new();
    let mut next = 0u64;
    let mut notification = [0; 64];
    let core_port = listener.local_addr().map_or(0, |a| a.port());
    let datagrams = mapper.as_ref().map(|mapper| {
        let mapper = mapper.clone();
        let acl = changed.clone();
        let (send, receive) = mpsc::channel(1);
        sessions.insert(0, (send, None));
        let ports = ports.clone();
        tokio::spawn(async move { mapper.datagrams(core_port, ports, acl, receive).await })
    });
    loop {
        tokio::select! {
            event = accept(&listener, &mapper, protocol), if changed.borrow().is_some() => {
                let Ok((socket, peer, protocol)) = event else { break };
                let Some(policy) = changed.borrow().clone() else { continue };
                if clients.len() >= MAX_CLIENTS || !permits(policy.clone(), Ok(peer)).await { continue; }
                next += 1;
                let (send, receive) = tokio::sync::mpsc::channel(1);
                let (acl, updates) = watch::channel(policy);
                clients.spawn(session::run(next, socket, ports.clone(), receive, updates, protocol));
                sessions.insert(next, (send, Some(acl)));
            }
            event = wake.recv(&mut notification) => {
                if event.is_err() { break; }
                ports.io_wake().drain();
                for reply in ports.replies().take(MAX_CLIENTS) {
                    if let Some((send, _)) = sessions.get(&reply.id().connection()) { let _ = send.try_send(reply); }
                }
                ports.notify_frame();
            }
            event = changed.changed() => {
                if event.is_err() { break; }
                if let Some(policy) = changed.borrow().clone() {
                    for acl in sessions.values().filter_map(|(_, acl)| acl.as_ref()) { acl.send_replace(policy.clone()); }
                }
            }
            Some(done) = clients.join_next(), if !clients.is_empty() => {
                if let Ok(id) = done { sessions.remove(&id); }
            }
        }
    }
    if let Some(datagrams) = datagrams {
        datagrams.abort();
        let _ = datagrams.await;
    }
    clients.abort_all();
    while clients.join_next().await.is_some() {}
}

async fn accept(
    listener: &TcpListener,
    mapper: &Option<Arc<super::rpc::Mapper>>,
    protocol: Protocol,
) -> io::Result<(tokio::net::TcpStream, std::net::SocketAddr, Protocol)> {
    tokio::select! {
        result = listener.accept() => result.map(|(s, p)| (s, p, protocol)),
        result = async {
            if let Some(mapper) = mapper { mapper.tcp.accept().await }
            else { std::future::pending().await }
        } => result.map(|(s, p)| (s, p, Protocol::Rpc(crate::netport::rpc::Channel::Mapper { port: listener.local_addr().map_or(0, |a| a.port()), datagram: false }))),
    }
}

// IPv6 on-link ACLs read a prefix file. Keep that filesystem work off the IO executor;
// The eight connection rechecks, one accept and one datagram bound the single blocking worker's queue.
pub(super) async fn permits(acl: crate::netacl::Shared, peer: io::Result<std::net::SocketAddr>) -> bool {
    tokio::task::spawn_blocking(move || acl.permits_peer(peer)).await.unwrap_or(false)
}
