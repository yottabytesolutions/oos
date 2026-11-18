// SPDX-License-Identifier: GPL-2.0-or-later
use super::port::{Reply, MAX_CLIENTS};
use super::*;
use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::collections::HashMap;
use tokio::{
    net::TcpListener,
    sync::{mpsc, watch},
};

pub(super) fn open(
    port: crate::net6::Port,
    v6: bool,
    ports: Arc<Ports>,
    changed: watch::Receiver<Option<crate::netacl::Shared>>,
    protocol: Protocol,
    stop: tokio_util::sync::CancellationToken,
) -> io::Result<(Bound, impl std::future::Future<Output = ()>)> {
    let listener = listen(port, v6)?;
    let mapper =
        if matches!(protocol, Protocol::Rpc(_)) { Some(Arc::new(super::rpc::Mapper::open(port, v6)?)) } else { None };
    let bound = Bound { core: listener.local_addr()?.port(), vxi: mapper.as_ref().map(|m| m.ports()).transpose()? };
    Ok((bound, async move {
        serve(listener, mapper, ports.clone(), changed, protocol, stop).await;
        ports.close();
    }))
}

pub(super) fn listen(port: crate::net6::Port, v6: bool) -> io::Result<TcpListener> {
    let listener = match port {
        crate::net6::Port::Host => std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).ok(),
        crate::net6::Port::Service(port) => bind_any(v6, port, std::net::TcpListener::bind),
    }
    .ok_or_else(|| {
        eprintln!("oos-ui: TCP listener {port:?} unavailable");
        io::Error::other("listener unavailable")
    })?;
    listener.set_nonblocking(true)?;
    TcpListener::from_std(listener)
}

async fn serve(
    listener: TcpListener,
    mapper: Option<Arc<super::rpc::Mapper>>,
    ports: Arc<Ports>,
    mut changed: watch::Receiver<Option<crate::netacl::Shared>>,
    protocol: Protocol,
    stop: tokio_util::sync::CancellationToken,
) {
    let sessions = Arc::new(ReplyRoutes::new(HashMap::new()));
    let core_port = listener.local_addr().map_or(0, |a| a.port());
    let (send, receive) = mpsc::channel(1);
    oos_hal::sync::lock(&sessions).insert(0, send);
    if tokio::select! {
        _ = stop.cancelled() => true,
        policy = changed.wait_for(Option::is_some) => policy.is_err(),
    } {
        return;
    }
    let datagrams = async {
        if let Some(mapper) = &mapper {
            mapper.datagrams(core_port, ports.clone(), changed.clone(), receive).await;
        }
        std::future::pending::<()>().await;
    };
    let mut next = 0;
    let serving = super::listener::serve(
        MAX_CLIENTS,
        stop.clone(),
        || accept(&listener, &mapper, protocol, changed.clone()),
        |(socket, peer, protocol), capacity| {
            next += 1;
            let id = next;
            let (ports, sessions, mut acl) = (ports.clone(), sessions.clone(), changed.clone());
            acl.mark_unchanged();
            async move {
                let policy = acl.borrow().clone()?;
                if matches!(capacity, super::listener::Capacity::Full) || !permits(policy, Ok(peer)).await {
                    return None;
                }
                let (send, receive) = mpsc::channel(1);
                oos_hal::sync::lock(&sessions).insert(id, send);
                Some(async move {
                    session::run(id, socket, ports, receive, acl, protocol).await;
                    oos_hal::sync::lock(&sessions).remove(&id);
                })
            }
        },
    );
    tokio::select! {
        _ = serving => {},
        _ = route(&ports, &sessions) => {},
        _ = datagrams => {},
    }
}

type ReplyRoutes = std::sync::Mutex<HashMap<u64, mpsc::Sender<Reply>>>;
async fn route(ports: &Ports, sessions: &ReplyRoutes) {
    loop {
        ports.replies().notified().await;
        for reply in ports.replies().take(MAX_CLIENTS) {
            if let Some(send) = oos_hal::sync::lock(sessions).get(&reply.id().connection()) {
                let _ = send.try_send(reply);
            }
        }
    }
}

async fn accept(
    listener: &TcpListener,
    mapper: &Option<Arc<super::rpc::Mapper>>,
    protocol: Protocol,
    mut changed: watch::Receiver<Option<crate::netacl::Shared>>,
) -> io::Result<(tokio::net::TcpStream, std::net::SocketAddr, Protocol)> {
    changed.mark_unchanged();
    loop {
        let enabled = changed.borrow().is_some();
        tokio::select! {
            biased;
            result = changed.changed() => { result.map_err(io::Error::other)?; }
            result = listener.accept(), if enabled => return result.map(|(s, p)| (s, p, protocol)),
            result = async {
                if let Some(mapper) = mapper { mapper.tcp.accept().await }
                else { std::future::pending().await }
            }, if enabled => {
                let (socket, peer) = result?;
                let channel = super::port::rpc::Channel::Mapper {
                    port: listener.local_addr().map_or(0, |a| a.port()), datagram: false,
                };
                return Ok((socket, peer, Protocol::Rpc(channel)));
            }
        }
    }
}

// IPv6 on-link ACLs read a prefix file. Keep that filesystem work off the IO executor;
// The eight connection rechecks, one accept and one datagram bound the single blocking worker's queue.
pub(super) async fn permits(acl: crate::netacl::Shared, peer: io::Result<std::net::SocketAddr>) -> bool {
    tokio::task::spawn_blocking(move || acl.permits_peer(peer)).await.unwrap_or(false)
}

/// `bind` on `[::]:port` when `v6`, else (or when IPv6 is not there) on `0.0.0.0:port`.
pub(super) fn bind_any<T>(v6: bool, port: u16, bind: impl Fn(SocketAddr) -> io::Result<T>) -> Option<T> {
    if v6 {
        if let Ok(s) = bind(SocketAddr::from((Ipv6Addr::UNSPECIFIED, port))) {
            return Some(s);
        }
    }
    bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port))).ok()
}

#[cfg(test)]
mod tests;
