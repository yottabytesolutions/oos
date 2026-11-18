// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::io::Write;

#[test]
fn hislip_clear_discards_only_bytes_buffered_before_the_next_read() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let mut client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (socket, _) = listener.accept().await.unwrap();
        let mut buffer = b"discard this pre-clear program".to_vec();
        let cleared = std::sync::atomic::AtomicBool::new(true);
        let message = crate::netport::hislip::encode(7, 0, 44, b"*IDN?\n");
        client.write_all(&message).unwrap();
        assert!(read(&socket, &mut buffer, MAX_PROGRAM, &cleared).await.unwrap());
        assert!(!cleared.load(Ordering::Acquire));
        while buffer.len() < message.len() {
            assert!(read(&socket, &mut buffer, MAX_PROGRAM, &cleared).await.unwrap());
        }
        assert_eq!(buffer, message, "a fresh command after the clear acknowledgement must survive");
        assert!(!cleared.load(Ordering::Acquire));
    });
}

#[test]
fn hislip_read_ahead_overflow_cancels_instead_of_holding_a_credit_forever() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await.unwrap();
        let mut client = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (socket, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0; MAX_PROGRAM + crate::netport::hislip::HEADER];
        buffer[..2].copy_from_slice(b"HS");
        buffer[2] = crate::netport::hislip::DATA_END;
        client.write_all(b"x").unwrap();
        let cleared = std::sync::atomic::AtomicBool::new(false);
        let result = tokio::time::timeout(
            crate::ui::lifecycle::test_wait::HANG_GUARD,
            read_clear(&socket, &mut buffer, &cleared),
        )
        .await
        .expect("read-ahead overflow must complete");
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    });
}
