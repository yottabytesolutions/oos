// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use std::{
    future::Future,
    sync::mpsc,
    task::{Context, Waker},
};

fn config(on: bool) -> Config {
    Config { on, name: "oos-test".into(), instance: "OOS test".into(), services: Vec::new(), v6: true }
}

#[test]
fn shutdown_cancels_idle_discovery_and_keeps_the_shared_executor_alive() {
    let runtime = Runtime::new().unwrap();
    let mut mdns = Mdns::new(runtime.clone(), Some(Vec::new()));
    mdns.set(config(false));
    assert!(mdns.control.is_none(), "off does not start discovery");
    mdns.set(config(true));
    mdns.control.take();
    mdns.task.take().unwrap().wait_stopped().expect("idle discovery stops before the shared deadline");
    assert!(mdns.control.is_none() && mdns.task.is_none());
    let (send, receive) = mpsc::channel();
    runtime.spawn(async move {
        send.send(()).unwrap();
    });
    receive.recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).expect("other services can still run");
    mdns.shutdown(Instant::now());
}

#[test]
fn switch_closes_sockets_and_control_cancellation_wakes_an_off_responder() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let _enter = runtime.enter();
    let mut discovery =
        Discovery { responder: Responder::new(1), socket: None, ifaces: Vec::new(), refresh: f64::NEG_INFINITY };
    discovery.configure(0.0, config(true));
    assert!(discovery.socket.is_some());
    discovery.tick(0.0, Some(&[]));
    assert_eq!(discovery.refresh, 2.0);
    discovery.tick(1.99, Some(&[]));
    assert_eq!(discovery.refresh, 2.0);
    discovery.tick(2.0, Some(&[]));
    assert_eq!(discovery.refresh, 4.0);
    discovery.configure(3.0, config(false));
    assert!(discovery.socket.is_none());
    assert_eq!(discovery.responder.state(), State::Off);
    for on in [true, false] {
        let (control, changed) = watch::channel(config(on));
        let mut work = Box::pin(discover(changed, Some(Vec::new()), tokio_util::sync::CancellationToken::new()));
        let mut cx = Context::from_waker(Waker::noop());
        assert!(work.as_mut().poll(&mut cx).is_pending());
        drop(control);
        assert!(work.as_mut().poll(&mut cx).is_ready());
    }
}

#[test]
fn executor_cancellation_wakes_an_off_responder() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let _enter = runtime.enter();
    let (_control, changed) = watch::channel(config(false));
    let stop = tokio_util::sync::CancellationToken::new();
    let mut work = std::pin::pin!(discover(changed, Some(Vec::new()), stop.clone()));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(work.as_mut().poll(&mut cx).is_pending());
    stop.cancel();
    assert!(work.as_mut().poll(&mut cx).is_ready());
}
