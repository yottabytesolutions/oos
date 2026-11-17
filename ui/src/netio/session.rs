// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::netport::{Envelope, RemoteRequest, Reply, MAX_PROGRAM};
use std::sync::atomic::Ordering;
use tokio::{
    net::TcpStream,
    sync::{mpsc, watch},
};

pub(super) async fn run(
    id: u64,
    mut socket: TcpStream,
    ports: Arc<Ports>,
    mut replies: mpsc::Receiver<Reply>,
    mut acl: watch::Receiver<crate::netacl::Shared>,
) -> u64 {
    let _ = socket.set_nodelay(true);
    let peer = socket.peer_addr().ok();
    let session = conversation(id, &mut socket, &ports, &mut replies);
    tokio::pin!(session);
    loop {
        tokio::select! {
            _ = &mut session => break,
            changed = acl.changed() => {
                if changed.is_err() { break; }
                let policy = acl.borrow().clone();
                if !super::runtime::permits(policy, peer.ok_or_else(|| io::Error::other("missing peer"))).await { break; }
            }
        }
    }
    id
}

async fn conversation(
    id: u64,
    socket: &mut TcpStream,
    ports: &Arc<Ports>,
    replies: &mut mpsc::Receiver<Reply>,
) -> io::Result<()> {
    let mut buffer = Vec::new();
    let mut request = 0;
    loop {
        let Some((text, accepted)) = read_program(socket, &mut buffer, ports).await? else { return Ok(()) };
        request += 1;
        let envelope = Envelope::program(id, request, text);
        let request_id = envelope.id();
        let read_closed = envelope.read_closed();
        let _credit = envelope.credit(ports.clone());
        let bytes = match envelope.request() {
            RemoteRequest::Program(text) => text.capacity(),
            _ => 0,
        };
        if ports.requests().submit(request_id, envelope, bytes).is_err() {
            return Ok(());
        }
        ports.enqueue().record(accepted);
        let mut probe = [0];
        let reply = tokio::select! {
            reply = replies.recv() => reply,
            result = socket.peek(&mut probe) => {
                if result? == 0 {
                    read_closed.store(true, Ordering::Release);
                    ports.notify_frame();
                }
                replies.recv().await
            }
        }
        .ok_or_else(|| io::Error::other("reply port closed"))?;
        if reply.id() != request_id {
            return Err(io::Error::other("stale reply"));
        }
        let start = tokio::time::Instant::now();
        super::output::send(socket, reply, || start.elapsed(), |deadline| tokio::time::sleep_until(start + deadline))
            .await?;
    }
}

async fn read_program(
    socket: &TcpStream,
    buffer: &mut Vec<u8>,
    ports: &Ports,
) -> io::Result<Option<(String, std::time::Instant)>> {
    loop {
        if let Some(end) = buffer.iter().position(|&b| b == b'\n') {
            let accepted = ports.enqueue().now();
            let text = String::from_utf8_lossy(&buffer[..=end]).into_owned();
            buffer.drain(..=end);
            return Ok(Some((text, accepted)));
        }
        if buffer.len() >= MAX_PROGRAM {
            return Err(io::Error::other("program too large"));
        }
        socket.readable().await?;
        let mut chunk = [0; 4096];
        let available = chunk.len().min(MAX_PROGRAM - buffer.len());
        match socket.try_read(&mut chunk[..available]) {
            Ok(0) => return Ok(None),
            Ok(n) => {
                buffer.reserve_exact(n);
                buffer.extend_from_slice(&chunk[..n]);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
    }
}
