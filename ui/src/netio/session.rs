// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::netport::{hislip, Envelope, RemoteRequest, Reply, MAX_PROGRAM};
use std::sync::atomic::{AtomicBool, Ordering};
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
    protocol: Protocol,
) -> u64 {
    let _ = socket.set_nodelay(true);
    let peer = socket.peer_addr().ok();
    let closed = Arc::new(AtomicBool::new(false));
    let _connection = Connection(closed.clone(), ports.clone());
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
    protocol: Protocol,
    closed: Arc<AtomicBool>,
) -> io::Result<()> {
    let mut buffer = Vec::new();
    let mut request = 0;
    let cleared = Arc::new(AtomicBool::new(false));
    loop {
        if cleared.swap(false, Ordering::AcqRel) {
            buffer.clear();
        }
        let Some((message, accepted)) = read_message(socket, &mut buffer, ports, protocol, &closed, &cleared).await?
        else {
            return Ok(());
        };
        request += 1;
        let envelope = Envelope::message(id, request, message);
        let request_id = envelope.id();
        let read_closed = envelope.read_closed();
        let _credit = envelope.credit(ports.clone());
        let clearable = matches!(envelope.request(), RemoteRequest::Hislip { message: Some(m), .. }
            if m.typ == hislip::DATA_END);
        let bytes = match envelope.request() {
            RemoteRequest::Program(text) => text.capacity(),
            RemoteRequest::Vxi11 { record, .. } => record.capacity(),
            RemoteRequest::Hislip { message, .. } => message.as_ref().map_or(0, |m| m.data.capacity()),
            _ => 0,
        };
        if ports.requests().submit(request_id, envelope, bytes).is_err() {
            return Ok(());
        }
        ports.enqueue().record(accepted);
        let reply = tokio::select! {
            biased;
            result = wait_for_input(socket, &mut buffer, &cleared, protocol, clearable) => {
                result?;
                if clearable { continue; }
                if !matches!(protocol, Protocol::Raw) { return Ok(()); }
                read_closed.store(true, Ordering::Release);
                ports.notify_frame();
                receive(replies, request_id, protocol).await?
            }
            reply = receive(replies, request_id, protocol) => reply?,
        };
        let close = reply.closes();
        let start = tokio::time::Instant::now();
        super::output::send(socket, reply, || start.elapsed(), |deadline| tokio::time::sleep_until(start + deadline))
            .await?;
        if close {
            return Ok(());
        }
    }
}

async fn read_message(
    socket: &mut TcpStream,
    buffer: &mut Vec<u8>,
    ports: &Ports,
    protocol: Protocol,
    closed: &Arc<AtomicBool>,
    cleared: &Arc<AtomicBool>,
) -> io::Result<Option<(RemoteRequest, std::time::Instant)>> {
    use crate::netport::rpc;
    loop {
        if let Protocol::Raw = protocol {
            if let Some(end) = buffer.iter().position(|&b| b == b'\n') {
                let accepted = ports.enqueue().now();
                let text = String::from_utf8_lossy(&buffer[..=end]).into_owned();
                buffer.drain(..=end);
                return Ok(Some((RemoteRequest::Program(text), accepted)));
            }
        } else if matches!(protocol, Protocol::Hislip) {
            let message = hislip::take_message(buffer);
            if !matches!(message, Ok(None)) {
                return Ok(Some((
                    RemoteRequest::Hislip {
                        message: message.ok().flatten(),
                        closed: closed.clone(),
                        cleared: cleared.clone(),
                    },
                    ports.enqueue().now(),
                )));
            }
        } else if let Some(record) = rpc::take_record(buffer) {
            let accepted = ports.enqueue().now();
            if let Protocol::Rpc(channel) = protocol {
                return Ok(Some((RemoteRequest::Vxi11 { record, channel, closed: closed.clone() }, accepted)));
            }
        }
        let limit = match protocol {
            Protocol::Raw => MAX_PROGRAM,
            Protocol::Rpc(_) => rpc::MAX_RECORD,
            Protocol::Hislip => MAX_PROGRAM + hislip::HEADER,
        };
        if buffer.len() >= limit {
            return Err(io::ErrorKind::InvalidData.into());
        }
        if !read(socket, buffer, limit, cleared).await? {
            return Ok(None);
        }
    }
}

async fn read(socket: &TcpStream, buffer: &mut Vec<u8>, limit: usize, cleared: &AtomicBool) -> io::Result<bool> {
    loop {
        socket.readable().await?;
        if cleared.swap(false, Ordering::AcqRel) {
            buffer.clear();
        }
        let mut chunk = [0; 4096];
        let available = chunk.len().min(limit - buffer.len());
        match socket.try_read(&mut chunk[..available]) {
            Ok(0) => return Ok(false),
            Ok(n) => {
                buffer.reserve_exact(n);
                buffer.extend_from_slice(&chunk[..n]);
                return Ok(true);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }
    }
}

async fn wait_for_input(
    socket: &TcpStream,
    buffer: &mut Vec<u8>,
    cleared: &AtomicBool,
    protocol: Protocol,
    clearable: bool,
) -> io::Result<()> {
    if clearable {
        read_clear(socket, buffer, cleared).await
    } else if matches!(protocol, Protocol::Raw) {
        raw_read_closed(socket, buffer, cleared).await
    } else {
        disconnected(socket).await
    }
}

// A pending raw program still watches for EOF behind pipelined data.
async fn raw_read_closed(socket: &TcpStream, buffer: &mut Vec<u8>, cleared: &AtomicBool) -> io::Result<()> {
    while buffer.len() < MAX_PROGRAM {
        if !read(socket, buffer, MAX_PROGRAM, cleared).await? {
            return Ok(());
        }
    }
    disconnected(socket).await
}

pub(super) async fn disconnected(socket: &TcpStream) -> io::Result<()> {
    let mut probe = [0];
    if socket.peek(&mut probe).await? != 0 {
        std::future::pending::<()>().await;
    }
    Ok(())
}

async fn receive(replies: &mut mpsc::Receiver<Reply>, id: Id, protocol: Protocol) -> io::Result<Reply> {
    loop {
        let reply = replies.recv().await.ok_or_else(|| io::Error::other("reply port closed"))?;
        if reply.id() == id {
            return Ok(reply);
        }
        if !matches!(protocol, Protocol::Hislip) {
            return Err(io::Error::other("stale reply"));
        }
    }
}

async fn read_clear(socket: &TcpStream, buffer: &mut Vec<u8>, cleared: &AtomicBool) -> io::Result<()> {
    use hislip::{ASYNC_DEVICE_CLEAR, DEVICE_CLEAR_COMPLETE, HEADER};
    while buffer.len() < HEADER {
        if !read(socket, buffer, HEADER, cleared).await? {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
    }
    if &buffer[..2] != b"HS" || !matches!(buffer[2], DEVICE_CLEAR_COMPLETE | ASYNC_DEVICE_CLEAR) {
        while buffer.len() < MAX_PROGRAM + HEADER {
            if !read(socket, buffer, MAX_PROGRAM + HEADER, cleared).await? {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
        }
        let _ = socket.peek(&mut [0]).await?;
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(())
}

struct Connection(Arc<AtomicBool>, Arc<Ports>);
impl Drop for Connection {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
        self.1.notify_frame();
    }
}

#[cfg(test)]
mod tests;
