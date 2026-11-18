// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::{
    cell::Cell,
    future::{pending, poll_fn},
    task::Poll,
};

#[test]
fn stalled_transfers_expire_only_when_the_injected_timeout_completes() {
    crate::netio::wlan::tests::run(async {
        let expired = Cell::new(false);
        let operation = pending::<io::Result<usize>>();
        let timeout = poll_fn(|cx| {
            if expired.replace(true) {
                Poll::Ready(())
            } else {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        });
        let result = timed(operation, timeout).await;
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::TimedOut);
        assert_eq!(timed(std::future::ready(Ok(7)), pending()).await.unwrap(), 7);
    });
}

/// A fake wpa_supplicant on a socket: the commands a join sends, with only hex in them.
#[test]
fn join_commands() {
    crate::netio::wlan::tests::run(async {
        let dir = PathBuf::from("/tmp").join(format!("oos-wlan-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("wlan0");
        let srv = UnixDatagram::bind(&p).unwrap();
        let t = std::thread::spawn(move || {
            let mut seen = Vec::new();
            let mut b = [0u8; 512];
            loop {
                let (n, from) = srv.recv_from(&mut b).unwrap();
                let c = String::from_utf8_lossy(&b[..n]).into_owned();
                let a = if c == "ADD_NETWORK" { "3\n" } else { "OK\n" };
                srv.send_to(a.as_bytes(), from.as_pathname().unwrap()).unwrap();
                let end = c == "SAVE_CONFIG";
                seen.push(c);
                if end {
                    return seen;
                }
            }
        });
        let c = Ctrl::open(p).await.unwrap();
        let id = c.add(b"a b\"\n", Some(&[0xab; 32])).await.unwrap();
        c.select(id).await.unwrap();
        c.save().await.unwrap();
        let seen = t.join().unwrap();
        assert_eq!(seen[1], "SET_NETWORK 3 ssid 612062220a");
        assert_eq!(seen[3], format!("SET_NETWORK 3 psk {}", "ab".repeat(32)));
        let own = c.own.clone().unwrap();
        c.close().await;
        assert!(!own.exists(), "closing control removes its socket path");
        let _ = std::fs::remove_dir_all(&dir);
    });
}

#[test]
fn a_waiting_control_reply_leaves_other_io_runnable_and_can_be_cancelled() {
    crate::netio::wlan::tests::run(async {
        let (sock, peer) = tokio::net::UnixDatagram::pair().unwrap();
        let ctrl = Ctrl { sock, own: None };
        let request = ctrl.ask("STATUS");
        let marker = async {
            let mut bytes = [0; 32];
            let n = peer.recv(&mut bytes).await.unwrap();
            assert_eq!(&bytes[..n], b"STATUS");
            // This task runs on the same current-thread executor while the reply is still absent.
            tokio::spawn(async { 7 }).await.unwrap()
        };
        tokio::select! {
            result = request => panic!("unanswered request completed: {result:?}"),
            result = marker => assert_eq!(result, 7),
        }
        ctrl.close().await;
    });
}

#[test]
fn partial_network_setup_failure_removes_the_unsaved_entry() {
    crate::netio::wlan::tests::run(async {
        let (sock, peer) = tokio::net::UnixDatagram::pair().unwrap();
        let ctrl = Ctrl { sock, own: None };
        let server = async {
            let mut bytes = [0; 128];
            for (command, answer) in [
                ("ADD_NETWORK", "3\n"),
                ("SET_NETWORK 3 ssid 4c6162", "OK\n"),
                ("SET_NETWORK 3 scan_ssid 1", "FAIL\n"),
                ("REMOVE_NETWORK 3", "OK\n"),
            ] {
                let n = peer.recv(&mut bytes).await.unwrap();
                assert_eq!(String::from_utf8_lossy(&bytes[..n]), command);
                peer.send(answer.as_bytes()).await.unwrap();
            }
        };
        let (result, ()) = tokio::join!(ctrl.add(b"Lab", None), server);
        assert_eq!(result.unwrap_err(), "SET_NETWORK: FAIL");
        ctrl.close().await;
    });
}
