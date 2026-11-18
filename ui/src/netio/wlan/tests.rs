// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::netport::Wake;
use control::Ctrl;
use operations::Clock;
use std::sync::Arc;
use std::{
    cell::Cell,
    future::Future,
    path::PathBuf,
    task::{Context, Poll, Waker},
};

pub(super) fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
}

pub(crate) fn link() -> Link {
    runtime().block_on(operations::link())
}
pub(crate) fn perform(job: Job) -> Report {
    runtime().block_on(operations::perform(
        job,
        &operations::Timer::new(),
        &Port::new(Arc::new(Wake::new().unwrap()), crate::store::sync_disks),
    ))
}

pub(super) fn run<T>(future: impl Future<Output = T>) -> T {
    runtime().block_on(async {
        tokio::time::timeout(crate::ui::lifecycle::test_wait::HANG_GUARD, future)
            .await
            .expect("WLAN fixture completion")
    })
}

#[derive(Default)]
struct ManualClock(Cell<Duration>);
impl Clock for ManualClock {
    fn now(&self) -> Duration {
        self.0.get()
    }
    async fn sleep(&self, duration: Duration) {
        self.0.set(self.0.get() + duration);
    }
}

struct Server {
    path: PathBuf,
    socket: tokio::net::UnixDatagram,
}
impl Server {
    fn new(name: &str) -> Self {
        let path = PathBuf::from("/tmp").join(format!("oos-wlan-test-{}-{name}", std::process::id()));
        let socket = tokio::net::UnixDatagram::bind(&path).unwrap();
        Self { path, socket }
    }
    async fn serve(&self, commands: &[(&str, &str)]) {
        let mut bytes = [0; 512];
        for (command, answer) in commands {
            let (n, from) = self.socket.recv_from(&mut bytes).await.unwrap();
            assert_eq!(String::from_utf8_lossy(&bytes[..n]), *command);
            self.socket.send_to(answer.as_bytes(), from.as_pathname().unwrap()).await.unwrap();
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).unwrap();
    }
}

#[test]
fn scan_busy_and_empty_results_keep_the_existing_retry_intervals() {
    run(async {
        for (name, first, elapsed) in
            [("scan-empty", "header\n", 7), ("scan-full", "header\nany\t2412\t-40\t[ESS]\tLab\n", 4)]
        {
            let server = Server::new(name);
            let c = Ctrl::open(server.path.clone()).await.unwrap();
            let mut commands = vec![("SCAN", "FAIL-BUSY\n"), ("SCAN_RESULTS", first)];
            if elapsed == 7 {
                commands.push(("SCAN_RESULTS", "header\nany\t2412\t-40\t[ESS]\tLab\n"));
            }
            let clock = ManualClock::default();
            let (result, ()) = tokio::join!(operations::scan(&c, &clock), server.serve(&commands));
            assert_eq!(result.unwrap()[0].name(), "Lab");
            assert_eq!(clock.now(), Duration::from_secs(elapsed));
            c.close().await;
        }
    });
}

#[test]
fn failed_new_join_reenables_saved_networks_and_removes_only_the_new_network() {
    run(async {
        let server = Server::new("join-failed");
        let c = Ctrl::open(server.path.clone()).await.unwrap();
        let clock = ManualClock::default();
        let commands = [
            ("ADD_NETWORK", "3\n"),
            ("SET_NETWORK 3 ssid 4c6162", "OK\n"),
            ("SET_NETWORK 3 scan_ssid 1", "OK\n"),
            ("SET_NETWORK 3 key_mgmt NONE", "OK\n"),
            ("SELECT_NETWORK 3", "OK\n"),
            ("STATUS", "wpa_state=DISCONNECTED\n"),
            ("LIST_NETWORKS", "header\n3\tLab\tany\t[TEMP-DISABLED]\n"),
            ("ENABLE_NETWORK all", "OK\n"),
            ("REMOVE_NETWORK 3", "OK\n"),
        ];
        let (result, ()) =
            tokio::join!(operations::join(&c, b"Lab".to_vec(), How::Open, &clock), server.serve(&commands));
        assert_eq!(result.unwrap_err(), "Could not join Lab: no answer");
        assert_eq!(clock.now(), Duration::from_millis(500));
        c.close().await;
    });
}

#[test]
fn saved_join_timeout_keeps_the_saved_network_and_uses_injected_time() {
    run(async {
        let server = Server::new("join-timeout");
        let c = Ctrl::open(server.path.clone()).await.unwrap();
        let clock = ManualClock::default();
        let mut commands = vec![("SELECT_NETWORK 3", "OK\n")];
        for _ in 0..61 {
            commands.extend([("STATUS", "wpa_state=DISCONNECTED\n"), ("LIST_NETWORKS", "header\n")]);
        }
        commands.push(("ENABLE_NETWORK all", "OK\n"));
        let (result, ()) =
            tokio::join!(operations::join(&c, b"Lab".to_vec(), How::Saved(3), &clock), server.serve(&commands));
        assert_eq!(result.unwrap_err(), "Could not join Lab: wrong password?");
        assert_eq!(clock.now(), Duration::from_millis(30500));
        c.close().await;
    });
}

#[test]
fn port_admission_and_report_backpressure_do_not_block_intake() {
    run(async {
        let (jobs, mut receive) = mpsc::channel(1);
        let wake = Arc::new(Wake::new().unwrap());
        let mut port = Wlan {
            runtime: Runtime::new().unwrap(),
            port: Port::new(wake.clone(), || {}),
            jobs: Some(jobs),
            reports: None,
        };
        assert!(port.send(Job::Scan));
        assert!(!port.send(Job::Forget(0)));
        assert!(matches!(receive.recv().await, Some(Job::Scan)));
        let (out, reports) = mpsc::channel(2);
        port.reports = Some(reports);
        for _ in 0..2 {
            assert!(publish(&out, &port.port, Report::Link(Link::default())).await);
        }
        assert!(wake.wait(0));
        wake.drain();
        let notify = port.port.clone();
        let mut pending = std::pin::pin!(publish(&out, &notify, Report::Done(Ok("done".into()))));
        assert!(matches!(pending.as_mut().poll(&mut Context::from_waker(Waker::noop())), Poll::Pending));
        assert!(matches!(port.poll(), Some(Report::Link(_))));
        assert!(pending.await);
        assert!(matches!(port.poll(), Some(Report::Link(_))));
        assert!(matches!(port.poll(), Some(Report::Done(Ok(_)))));
        drop(port);
        assert!(!publish(&out, &notify, Report::Link(Link::default())).await);
    });
}

#[test]
fn status_reports_follow_link_changes_and_request_signal_only_while_connected() {
    run(async {
        let server = Server::new("link");
        let c = Ctrl::open(server.path.clone()).await.unwrap();
        let commands = [
            ("STATUS", "wpa_state=COMPLETED\nssid=Lab\nip_address=192.0.2.1\nfreq=2412\n"),
            ("SIGNAL_POLL", "RSSI=-42\n"),
            ("LIST_NETWORKS", "header\n3\tLab\tany\t[CURRENT]\n"),
            ("STATUS", "wpa_state=DISCONNECTED\n"),
            ("LIST_NETWORKS", "header\n"),
        ];
        let read = async {
            let linked = operations::read_link(&c).await;
            assert!(linked.stick() && linked.ctrl() && linked.status().connected());
            assert_eq!(linked.status().ssid, b"Lab");
            assert_eq!(linked.status().ip, "192.0.2.1");
            assert_eq!(linked.signal(), Some(-42));
            assert_eq!(linked.saved()[0].id, 3);
            let lost = operations::read_link(&c).await;
            assert!(lost.stick() && lost.ctrl() && !lost.status().connected());
            assert_eq!(lost.signal(), None);
            assert!(lost.saved().is_empty());
        };
        tokio::join!(read, server.serve(&commands));
        drop(server);
        let unavailable = operations::read_link(&c).await;
        assert!(unavailable.stick() && !unavailable.ctrl());
        assert_eq!(unavailable.status(), &wlan::Status::default());
        assert_eq!(unavailable.signal(), None);
        assert!(unavailable.saved().is_empty());
        c.close().await;
    });
}

#[test]
fn successful_join_reenables_saved_networks_before_saving_and_syncing() {
    run(async {
        let server = Server::new("join-ok");
        let c = Ctrl::open(server.path.clone()).await.unwrap();
        let clock = ManualClock::default();
        let commands = [
            ("SELECT_NETWORK 3", "OK\n"),
            ("STATUS", "wpa_state=COMPLETED\nssid=Lab\n"),
            ("ENABLE_NETWORK all", "OK\n"),
            ("SAVE_CONFIG", "OK\n"),
        ];
        let join = async {
            assert_eq!(operations::join(&c, b"Lab".to_vec(), How::Saved(3), &clock).await.unwrap(), "Connected to Lab");
            operations::saved_to_card(&c, Port::new(Arc::new(Wake::new().unwrap()), || {})).await.unwrap();
        };
        tokio::join!(join, server.serve(&commands));
        assert_eq!(clock.now(), Duration::from_millis(500));
        c.close().await;
    });
}

#[test]
fn rejected_save_does_not_sync_disks() {
    run(async {
        let server = Server::new("save-failed");
        let c = Ctrl::open(server.path.clone()).await.unwrap();
        let port = Port::new(Arc::new(Wake::new().unwrap()), || panic!("failed save must not sync"));
        let (result, ()) =
            tokio::join!(operations::saved_to_card(&c, port), server.serve(&[("SAVE_CONFIG", "FAIL\n")]));
        assert_eq!(result.unwrap_err(), "SAVE_CONFIG: FAIL");
        c.close().await;
    });
}

#[test]
fn oversized_jobs_are_rejected_and_retained_strings_have_bounded_capacity() {
    run(async {
        let (send, mut receive) = mpsc::channel(1);
        let port = Wlan {
            runtime: Runtime::new().unwrap(),
            port: Port::new(Arc::new(Wake::new().unwrap()), || {}),
            jobs: Some(send),
            reports: None,
        };
        assert!(!port.send(Job::Join(vec![0; wlan::SSID_MAX + 1], How::Open)));
        assert!(!port.send(Job::Join(b"Lab".to_vec(), How::Pass("x".repeat(wlan::PASS_LEN.1 + 1)))));
        let mut ssid = Vec::with_capacity(4096);
        ssid.extend_from_slice(b"Lab");
        let mut pass = String::with_capacity(4096);
        pass.push_str("password");
        assert!(port.send(Job::Join(ssid, How::Pass(pass))));
        let Some(Job::Join(ssid, How::Pass(pass))) = receive.recv().await else { panic!("join accepted") };
        assert_eq!(ssid.capacity(), ssid.len());
        assert_eq!(pass.capacity(), pass.len());
    });
}
