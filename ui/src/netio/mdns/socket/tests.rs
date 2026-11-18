// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;

#[test]
fn queued_discovery_packets_let_other_tasks_progress() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        for v6 in [false, true] {
            let receiver = open(v6, 0).unwrap();
            opt(receiver.as_raw_fd(), libc::SOL_SOCKET, libc::SO_RCVBUF, 1 << 20).unwrap();
            let port = receiver.local_addr().unwrap().port();
            let destination =
                SocketAddr::new(if v6 { Ipv6Addr::LOCALHOST.into() } else { Ipv4Addr::LOCALHOST.into() }, port);
            let sender = UdpSocket::bind(SocketAddr::new(destination.ip(), 0)).unwrap();
            let receiver = register(receiver).unwrap();
            let mut net = Net { s4: receiver, s6: None, joined4: Vec::new(), joined6: Vec::new() };
            if v6 {
                net.s6 = Some(std::mem::replace(&mut net.s4, register(open(false, 0).unwrap()).unwrap()));
            }
            for sequence in 0..192u8 {
                sender.send_to(&[sequence], destination).unwrap();
            }
            let socket = net.s6.as_ref().unwrap_or(&net.s4);
            // Cache readiness before polling discovery ahead of the other runnable work.
            drop(socket.readable().await.unwrap());
            let progressed = std::cell::Cell::new(false);
            tokio::time::timeout(crate::ui::lifecycle::test_wait::HANG_GUARD, async {
                tokio::join!(
                    biased;
                    async {
                        for sequence in 0..192u8 {
                            let mut buf = [0; 1];
                            let (n, _, _, _) = net.recv(&mut buf).await.unwrap();
                            assert_eq!((n, buf[0]), (1, sequence));
                        }
                        assert!(progressed.get(), "discovery drained the queue before other runnable work");
                    },
                    async { progressed.set(true) },
                );
            })
            .await
            .expect("queued discovery packets drain");
        }
    });
}

#[test]
fn readiness_receives_interface_destination_and_sender_for_both_families() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        for v6 in [false, true] {
            let receiver = open(v6, 0).unwrap();
            let port = receiver.local_addr().unwrap().port();
            let destination = if v6 {
                SocketAddr::from((Ipv6Addr::LOCALHOST, port))
            } else {
                SocketAddr::from((Ipv4Addr::LOCALHOST, port))
            };
            let sender = UdpSocket::bind(SocketAddr::new(destination.ip(), 0)).unwrap();
            let receiver = register(receiver).unwrap();
            assert_eq!(recv_on(receiver.get_ref(), &mut [0; 64]).unwrap_err().kind(), io::ErrorKind::WouldBlock);
            let mut net = Net { s4: receiver, s6: None, joined4: Vec::new(), joined6: Vec::new() };
            // Route the second receive through the optional IPv6 readiness branch as well.
            if v6 {
                net.s6 = Some(std::mem::replace(&mut net.s4, register(open(false, 0).unwrap()).unwrap()));
            }
            sender.send_to(b"packet metadata", destination).unwrap();
            let mut buf = [0; 64];
            let (n, peer, index, group) =
                tokio::time::timeout(crate::ui::lifecycle::test_wait::HANG_GUARD, net.recv(&mut buf))
                    .await
                    .expect("packet readiness")
                    .unwrap();
            assert_eq!(&buf[..n], b"packet metadata");
            assert_eq!(peer, sender.local_addr().unwrap());
            assert_ne!(index, 0);
            assert!(!group, "loopback unicast destination");
        }
    });
}
