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
    protocol: super::rpc::Protocol,
) -> u64 {
    let _ = socket.set_nodelay(true);
    let peer = socket.peer_addr().ok();
    let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let _connection = super::rpc::Connection(closed.clone(), ports.clone());
    let session = conversation(id, &mut socket, &ports, &mut replies, protocol, closed);
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
    protocol: super::rpc::Protocol,
    closed: Arc<std::sync::atomic::AtomicBool>,
) -> io::Result<()> {
    let mut buffer = Vec::new();
    let mut request = 0;
    loop {
        let Some((message, accepted)) = read_message(socket, &mut buffer, ports, protocol, &closed).await? else {
            return Ok(());
        };
        request += 1;
        let envelope = Envelope::message(id, request, message);
        let request_id = envelope.id();
        let read_closed = envelope.read_closed();
        let _credit = envelope.credit(ports.clone());
        let bytes = match envelope.request() {
            RemoteRequest::Program(text) => text.capacity(),
            RemoteRequest::Vxi11 { record, .. } => record.capacity(),
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
                    if matches!(protocol, super::rpc::Protocol::Rpc(_)) { return Ok(()); }
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

async fn read_message(
    socket: &mut TcpStream,
    buffer: &mut Vec<u8>,
    ports: &Ports,
    protocol: super::rpc::Protocol,
    closed: &Arc<std::sync::atomic::AtomicBool>,
) -> io::Result<Option<(RemoteRequest, std::time::Instant)>> {
    use super::rpc::Protocol;
    use crate::netport::rpc;
    loop {
        if let Protocol::Raw = protocol {
            if let Some(end) = buffer.iter().position(|&b| b == b'\n') {
                let accepted = ports.enqueue().now();
                let text = String::from_utf8_lossy(&buffer[..=end]).into_owned();
                buffer.drain(..=end);
                return Ok(Some((RemoteRequest::Program(text), accepted)));
            }
        } else if let Some(record) = rpc::take_record(buffer) {
            let accepted = ports.enqueue().now();
            if let Protocol::Rpc(channel) = protocol {
                return Ok(Some((RemoteRequest::Vxi11 { record, channel, closed: closed.clone() }, accepted)));
            }
        }
        let limit = if matches!(protocol, Protocol::Raw) { MAX_PROGRAM } else { rpc::MAX_RECORD };
        if buffer.len() >= limit {
            return Err(io::ErrorKind::InvalidData.into());
        }
        socket.readable().await?;
        let mut chunk = [0; 4096];
        let available = chunk.len().min(limit - buffer.len());
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
