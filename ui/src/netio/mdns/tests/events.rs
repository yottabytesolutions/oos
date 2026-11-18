// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use mdns_sd::RRType;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    task::{Context, Waker},
};

fn discovery() -> Discovery {
    Discovery { name: config().name, joined: ["192.0.2.1".parse().unwrap()].into(), announced: true }
}

fn conflict(rr_type: RRType, new_name: &str) -> DaemonEvent {
    DaemonEvent::NameChange(mdns_sd::DnsNameChange {
        original: "oos-test.local.".into(),
        new_name: new_name.into(),
        rr_type,
        intf_name: "test-interface".into(),
    })
}

#[test]
fn host_conflicts_remove_only_one_local_suffix() {
    for rr_type in [RRType::A, RRType::AAAA] {
        let mut discovery = discovery();
        discovery.event(conflict(rr_type, "example.local.local.")).unwrap();
        assert_eq!(discovery.status(), Status { state: State::Probing, name: "example.local".into() });
    }
}

#[test]
fn announcements_remove_only_one_suffix_and_address_lists_preserve_the_hostname() {
    let mut discovery = discovery();
    discovery.event(DaemonEvent::Announce(String::new(), "example.local.local.:test-interface".into())).unwrap();
    assert_eq!(discovery.status(), Status { state: State::Running, name: "example.local".into() });
    discovery.event(DaemonEvent::Announce(String::new(), "[2001:db8::1]".into())).unwrap();
    assert_eq!(discovery.status().name, "example.local");
}

#[test]
fn instance_conflicts_restart_probing_without_replacing_the_hostname() {
    for rr_type in [RRType::SRV, RRType::PTR] {
        let mut discovery = discovery();
        discovery.event(conflict(rr_type, "OOS test (2)._http._tcp.local.")).unwrap();
        assert_eq!(discovery.status(), Status { state: State::Probing, name: config().name });
    }
}

#[test]
fn address_events_override_lagging_snapshots_and_reconnection_needs_an_announcement() {
    let first = "192.0.2.1".parse().unwrap();
    let second = "2001:db8::1".parse().unwrap();
    let mut discovery = discovery();
    discovery.update_addresses(&DaemonEvent::IpAdd(second), [first, second].into());
    assert_eq!(discovery.status().state, State::Running);
    discovery.update_addresses(&DaemonEvent::IpDel(first), [first, second].into());
    assert_eq!(discovery.status().state, State::Running);
    discovery.update_addresses(&DaemonEvent::IpDel(second), [first, second].into());
    assert_eq!(discovery.status().state, State::NoNetwork);
    discovery.update_addresses(&DaemonEvent::IpAdd(first), [first].into());
    assert_eq!(discovery.status().state, State::Probing);
    discovery.event(DaemonEvent::Announce(String::new(), "[192.0.2.1]".into())).unwrap();
    assert_eq!(discovery.status(), Status { state: State::Running, name: config().name });
    discovery.update_addresses(&DaemonEvent::IpAdd(second), HashSet::new());
    assert_eq!(discovery.status().state, State::NoNetwork);
}

#[test]
fn errors_request_a_restart() {
    assert!(discovery().event(DaemonEvent::Error(mdns_sd::Error::Msg("socket unavailable".into()))).is_err());
}

#[tokio::test(start_paused = true)]
async fn failures_retry_after_backoff_without_a_configuration_change() {
    let (control, changed) = watch::channel(config());
    let mut control = Some(control);
    let attempts = Cell::new(0);
    let statuses = RefCell::new(Vec::new());
    let run = run_with(
        changed,
        Some(Vec::new()),
        || {
            if attempts.get() > 0 {
                assert_eq!(statuses.borrow().last(), Some(&State::Failed));
            }
            attempts.set(attempts.get() + 1);
            if attempts.get() == 2 {
                control.take();
            }
            Err(mdns_sd::Error::Msg("daemon unavailable".into()))
        },
        |status| statuses.borrow_mut().push(status.state),
    );
    let mut run = std::pin::pin!(run);
    let mut cx = Context::from_waker(Waker::noop());
    assert!(run.as_mut().poll(&mut cx).is_pending());
    assert_eq!(attempts.get(), 1);
    assert_eq!(*statuses.borrow(), [State::Failed]);
    tokio::time::advance(RETRY_DELAY - Duration::from_nanos(1)).await;
    assert!(run.as_mut().poll(&mut cx).is_pending());
    assert_eq!(attempts.get(), 1);
    tokio::time::advance(Duration::from_nanos(1)).await;
    assert!(run.as_mut().poll(&mut cx).is_ready());
    assert_eq!(attempts.get(), 2);
    assert_eq!(*statuses.borrow(), [State::Failed, State::Failed, State::Off]);
}

#[tokio::test(start_paused = true)]
async fn switch_off_and_shutdown_cancel_stalled_discovery() {
    for shutdown in [false, true] {
        let (control, mut changed) = watch::channel(config());
        if shutdown {
            drop(control);
        } else {
            control.send_replace(Config { on: false, ..config() });
        }
        let discovery = until_changed(&mut changed, std::future::pending());
        let mut discovery = std::pin::pin!(discovery);
        let result = discovery.as_mut().poll(&mut Context::from_waker(Waker::noop()));
        assert!(matches!(result, std::task::Poll::Ready(Ok(update)) if update == !shutdown));
    }
}

#[tokio::test(start_paused = true)]
async fn switch_off_interrupts_failure_backoff_and_shutdown_stops_the_worker() {
    let (control, changed) = watch::channel(config());
    let attempts = Cell::new(0);
    let statuses = RefCell::new(Vec::new());
    let run = run_with(
        changed,
        Some(Vec::new()),
        || {
            if attempts.get() > 0 {
                assert_eq!(statuses.borrow().last(), Some(&State::Failed));
            }
            attempts.set(attempts.get() + 1);
            Err(mdns_sd::Error::Msg("daemon unavailable".into()))
        },
        |status| statuses.borrow_mut().push(status.state),
    );
    let mut run = std::pin::pin!(run);
    let mut cx = Context::from_waker(Waker::noop());
    assert!(run.as_mut().poll(&mut cx).is_pending());
    control.send_replace(Config { on: false, ..config() });
    assert!(run.as_mut().poll(&mut cx).is_pending());
    assert_eq!(attempts.get(), 1);
    assert_eq!(*statuses.borrow(), [State::Failed, State::Off]);
    drop(control);
    assert!(run.as_mut().poll(&mut cx).is_ready());
}

#[tokio::test(start_paused = true)]
async fn advertisement_errors_also_restart_the_daemon() {
    let (control, changed) = watch::channel(config());
    let mut control = Some(control);
    let attempts = Cell::new(0);
    run_with(
        changed,
        Some(Vec::new()),
        || {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 2 {
                control.take();
                return Err(mdns_sd::Error::Msg("daemon unavailable".into()));
            }
            let daemon = ServiceDaemon::new_with_port(0).unwrap();
            daemon.shutdown().unwrap().recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
            // A closed daemon makes monitor() in advertise fail rather than construction.
            Ok(daemon)
        },
        |_| {},
    )
    .await;
    assert_eq!(attempts.get(), 2);
}

#[tokio::test]
async fn daemon_replies_have_a_bounded_timeout() {
    let daemon = ServiceDaemon::new_with_port(0).unwrap();
    daemon.disable_interface(IfKind::All).unwrap();
    let events = daemon.monitor().unwrap();
    daemon.status().unwrap().recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD).unwrap();
    tokio::time::pause();
    assert!(matches!(daemon_reply(Ok(events)).await, Err(DiscoveryError::Timeout)));
    tokio::time::resume();
    assert_eq!(daemon_reply(daemon.shutdown()).await.unwrap(), DaemonStatus::Shutdown);
}

#[test]
fn probe_announcement_extracts_the_host_label() {
    assert_eq!(announced_host("example.local.:test-interface"), Some("example"));
    assert_eq!(announced_host("example.local.local.:test-interface"), Some("example.local"));
}

#[test]
fn ipv4_registration_address_list_has_no_host_label() {
    assert_eq!(announced_host("[192.0.2.1, 198.51.100.1]"), None);
}

#[test]
fn ipv6_registration_address_list_has_no_host_label() {
    assert_eq!(announced_host("[2001:db8::1]"), None);
}

#[test]
fn mixed_and_empty_registration_address_lists_have_no_host_label() {
    assert_eq!(announced_host("[192.0.2.1, 2001:db8::1]"), None);
    assert_eq!(announced_host("[]"), None);
}

#[test]
fn incomplete_probe_announcement_has_no_host_label() {
    for address in ["example.local.", "example.local.:", "example:test-interface", "2001:db8::1"] {
        assert_eq!(announced_host(address), None);
    }
    assert_eq!(host_label("example"), "example");
}

#[tokio::test]
async fn address_events_refresh_membership_inside_the_event_handler() {
    let daemon = ServiceDaemon::new_with_port(0).unwrap();
    daemon.disable_interface(IfKind::All).unwrap();
    let mut discovery = discovery();
    on_event(&daemon, &mut discovery, DaemonEvent::IpAdd("2001:db8::1".parse().unwrap())).await.unwrap();
    assert_eq!(discovery.status().state, State::NoNetwork);
    assert!(!discovery.announced);
    daemon_reply(daemon.shutdown()).await.unwrap();
}

#[tokio::test]
async fn reply_errors_preserve_daemon_failures_and_closed_channels() {
    let error = daemon_reply::<()>(Err(mdns_sd::Error::Msg("daemon unavailable".into()))).await.unwrap_err();
    assert!(matches!(error, DiscoveryError::Daemon(mdns_sd::Error::Msg(_))));
    let daemon = ServiceDaemon::new_with_port(0).unwrap();
    daemon.disable_interface(IfKind::All).unwrap();
    let events = daemon.monitor().unwrap();
    daemon_reply(daemon.shutdown()).await.unwrap();
    assert!(matches!(daemon_reply(Ok(events)).await, Err(DiscoveryError::Closed)));
}
