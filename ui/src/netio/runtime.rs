// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::netport::{Reply, MAX_CLIENTS};
use std::collections::HashMap;
use tokio::{
    net::{TcpListener, UnixDatagram},
    sync::{mpsc, watch},
    task::JoinSet,
};

pub(super) fn run(
    port: crate::net6::Port,
    v6: bool,
    ports: Arc<Ports>,
    changed: watch::Receiver<Option<crate::netacl::Shared>>,
    ready: std::sync::mpsc::SyncSender<io::Result<u16>>,
) {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().max_blocking_threads(1).build();
    let result = runtime.and_then(|runtime| {
        runtime.block_on(async {
            let listener = crate::net6::tcp(port, v6).ok_or_else(|| io::Error::other("SCPI listener unavailable"))?;
            listener.set_nonblocking(true)?;
            let listener = TcpListener::from_std(listener)?;
            let wake = UnixDatagram::from_std(ports.io_wake().receiver()?)?;
            let port = listener.local_addr()?.port();
            let _ = ready.send(Ok(port));
            serve(listener, wake, ports.clone(), changed).await;
            Ok(())
        })
    });
    if let Err(e) = result {
        let _ = ready.send(Err(e));
    }
    ports.close();
}

async fn serve(
    listener: TcpListener,
    wake: UnixDatagram,
    ports: Arc<Ports>,
    mut changed: watch::Receiver<Option<crate::netacl::Shared>>,
) {
    let mut clients = JoinSet::new();
    let mut sessions: HashMap<u64, (mpsc::Sender<Reply>, watch::Sender<crate::netacl::Shared>)> = HashMap::new();
    let mut next = 0u64;
    let mut notification = [0; 64];
    loop {
        tokio::select! {
            event = listener.accept(), if changed.borrow().is_some() => {
                let Ok((socket, peer)) = event else { break };
                if clients.len() >= MAX_CLIENTS { continue; }
                let Some(policy) = changed.borrow().clone() else { continue };
                if !permits(policy.clone(), Ok(peer)).await { continue; }
                next += 1;
                let (send, receive) = tokio::sync::mpsc::channel(1);
                let (acl, updates) = watch::channel(policy);
                // Each task owns its one-credit reply mailbox and cancellation guard.
                clients.spawn(session::run(next, socket, ports.clone(), receive, updates));
                sessions.insert(next, (send, acl));
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
                    for (_, acl) in sessions.values() { acl.send_replace(policy.clone()); }
                }
            }
            Some(done) = clients.join_next(), if !clients.is_empty() => {
                if let Ok(id) = done { sessions.remove(&id); }
            }
        }
    }
    clients.abort_all();
    while clients.join_next().await.is_some() {}
}

// IPv6 on-link ACLs read a prefix file. Keep that filesystem work off the IO executor;
// at most the eight connection rechecks and one accept can queue on the single blocking worker.
pub(super) async fn permits(acl: crate::netacl::Shared, peer: io::Result<std::net::SocketAddr>) -> bool {
    tokio::task::spawn_blocking(move || acl.permits_peer(peer)).await.unwrap_or(false)
}
