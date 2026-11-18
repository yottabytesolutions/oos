// SPDX-License-Identifier: GPL-2.0-or-later
//! ONC RPC framing and portmapper IO; link state and SCPI stay on the frame thread.
use super::*;
use crate::netport::rpc::Channel;
use tokio::net::{TcpListener, UdpSocket};

#[derive(Clone, Copy)]
pub(super) enum Protocol {
    Raw,
    Rpc(Channel),
}
pub(super) struct Mapper {
    pub(super) tcp: TcpListener,
    udp: UdpSocket,
}
impl Mapper {
    pub(super) fn open(port: crate::net6::Port, v6: bool) -> io::Result<Self> {
        use crate::net6::Port;
        let tcp = runtime::listen(
            match port {
                Port::Host => Port::Host,
                _ => Port::Service(111),
            },
            v6,
        )?;
        let udp = match port {
            Port::Host => std::net::UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0)).ok(),
            _ => crate::net6::udp(111, v6),
        }
        .ok_or_else(|| io::Error::other("portmapper unavailable"))?;
        udp.set_nonblocking(true)?;
        Ok(Self { tcp, udp: UdpSocket::from_std(udp)? })
    }
    pub(super) fn ports(&self) -> io::Result<(u16, u16)> {
        Ok((self.tcp.local_addr()?.port(), self.udp.local_addr()?.port()))
    }
    pub(super) async fn datagrams(
        &self,
        port: u16,
        ports: Arc<Ports>,
        acl: tokio::sync::watch::Receiver<Option<crate::netacl::Shared>>,
        mut replies: tokio::sync::mpsc::Receiver<Reply>,
    ) {
        let mut buffer = [0; 1500];
        let mut sequence = 0;
        loop {
            let Ok((n, peer)) = self.udp.recv_from(&mut buffer).await else { return };
            let Some(policy) = acl.borrow().clone() else { continue };
            if !runtime::permits(policy, Ok(peer)).await {
                continue;
            }
            sequence += 1;
            let request = Envelope::message(
                0,
                sequence,
                RemoteRequest::Vxi11 {
                    record: buffer[..n].to_vec(),
                    channel: Channel::Mapper { port, datagram: true },
                    closed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
                },
            );
            let _credit = request.credit(ports.clone());
            let id = request.id();
            if ports.requests().submit(id, request, n).is_err() {
                continue;
            }
            let Some(reply) = replies.recv().await else { return };
            if reply.id() == id {
                if let Some(bytes) = reply.output().chunks().next() {
                    let _ = self.udp.send_to(bytes, peer).await;
                }
            }
        }
    }
}

pub(super) struct Connection(pub Arc<std::sync::atomic::AtomicBool>, pub Arc<Ports>);
impl Drop for Connection {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Release);
        self.1.notify_frame();
    }
}
