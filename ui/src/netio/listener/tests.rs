// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

struct Session(Arc<AtomicBool>);
impl Drop for Session {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn cancellation_interrupts_admission_and_joins_existing_sessions() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let stop = CancellationToken::new();
        let closed = Arc::new(AtomicBool::new(false));
        let (started, mut events) = mpsc::channel(2);
        let mut next = 0;
        let serving = serve(
            1,
            stop.clone(),
            || std::future::ready(Ok(())),
            |(), capacity| {
                next += 1;
                let (started, closed, id) = (started.clone(), closed.clone(), next);
                async move {
                    if matches!(capacity, Capacity::Full) {
                        started.send(2).await.unwrap();
                        std::future::pending::<()>().await;
                    }
                    Some(async move {
                        let _session = Session(closed);
                        started.send(id).await.unwrap();
                        std::future::pending::<()>().await;
                    })
                }
            },
        );
        let cancel = async {
            for _ in 0..2 {
                events.recv().await.unwrap();
            }
            stop.cancel();
        };
        tokio::join!(serving, cancel);
        assert!(closed.load(Ordering::SeqCst), "shutdown acknowledges only after connection cleanup");
    });
}

#[test]
fn transient_accept_errors_leave_the_listener_available() {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
        let stop = CancellationToken::new();
        let mut events = [libc::EMFILE, libc::ENFILE, libc::ECONNABORTED]
            .into_iter()
            .map(|code| Err(io::Error::from_raw_os_error(code)))
            .chain(std::iter::once(Ok(())));
        let mut admitted = false;
        serve(
            1,
            stop.clone(),
            || {
                let event = events.next();
                async move {
                    match event {
                        Some(event) => event,
                        None => std::future::pending().await,
                    }
                }
            },
            |(), capacity| {
                assert!(matches!(capacity, Capacity::Available));
                admitted = true;
                stop.cancel();
                std::future::ready(None::<std::future::Ready<()>>)
            },
        )
        .await;
        assert!(admitted, "accept errors must not stop admission of the next connection");
    });
}
