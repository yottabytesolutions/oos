// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

use super::{
    device::{Device, Port},
    Handoff,
};
use std::cell::RefCell;
use std::io::{self, Write};
use std::rc::Rc;
use std::time::{Duration, Instant};

struct State {
    present: bool,
    pid: Option<String>,
    cmdline: Option<Vec<u8>>,
    request: Option<String>,
    signal_error: Option<i32>,
    alive_error: Option<i32>,
    busy: usize,
    open_error: Option<i32>,
    remove_error: Option<i32>,
    calls: Vec<String>,
    elapsed: Duration,
    ack: Ack,
}

#[derive(Default)]
struct Ack {
    value: Option<String>,
    after: Option<Duration>,
    reply: String,
    error: Option<i32>,
}

struct Fake {
    state: Rc<RefCell<State>>,
    start: Instant,
}

struct Handle(Rc<RefCell<State>>);

impl Write for Handle {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        assert_eq!(bytes, &[0], "never magic close");
        self.0.borrow_mut().calls.push("ping".into());
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.0.borrow_mut().calls.push("close".into());
    }
}

fn result(error: Option<i32>) -> io::Result<()> {
    error.map_or(Ok(()), |n| Err(io::Error::from_raw_os_error(n)))
}

impl Port for Fake {
    type Handle = Handle;

    fn present(&self) -> io::Result<bool> {
        Ok(self.state.borrow().present)
    }

    fn pid(&self) -> io::Result<String> {
        self.state.borrow().pid.clone().ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn cmdline(&self, pid: libc::pid_t) -> io::Result<Vec<u8>> {
        assert_eq!(pid, 42);
        self.state.borrow().cmdline.clone().ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }

    fn requester(&self) -> io::Result<Option<String>> {
        Ok(self.state.borrow().request.clone())
    }

    fn acknowledgement(&self) -> io::Result<Option<String>> {
        let mut s = self.state.borrow_mut();
        if s.calls.iter().any(|c| c == "signal") {
            result(s.ack.error)?;
            if s.ack.after.is_some_and(|t| s.elapsed >= t) {
                s.ack.value = Some(s.ack.reply.clone());
            }
        }
        Ok(s.ack.value.clone())
    }

    fn request(&self) -> io::Result<String> {
        let mut s = self.state.borrow_mut();
        if s.request.is_some() {
            return Err(io::ErrorKind::AlreadyExists.into());
        }
        s.calls.push("request".into());
        s.request = Some("99\n".into());
        Ok("99\n".into())
    }

    fn remove_request(&self) -> io::Result<()> {
        let mut s = self.state.borrow_mut();
        s.calls.push("remove".into());
        result(s.remove_error)?;
        s.request = None;
        Ok(())
    }

    fn signal(&self, pid: libc::pid_t, signal: libc::c_int) -> io::Result<()> {
        let mut s = self.state.borrow_mut();
        if signal == 0 {
            assert_eq!(pid, 99);
            s.calls.push("probe".into());
            result(s.alive_error)
        } else {
            assert_eq!((pid, signal), (42, libc::SIGUSR1));
            s.calls.push("signal".into());
            result(s.signal_error)
        }
    }

    fn open(&self) -> io::Result<Handle> {
        let mut s = self.state.borrow_mut();
        s.calls.push("open".into());
        if s.busy > 0 {
            s.busy -= 1;
            return Err(io::Error::from_raw_os_error(libc::EBUSY));
        }
        result(s.open_error)?;
        Ok(Handle(self.state.clone()))
    }

    fn now(&self) -> Instant {
        self.start + self.state.borrow().elapsed
    }

    fn pause(&self) {
        self.state.borrow_mut().elapsed += Duration::from_millis(10);
    }

    fn report(&self, message: &str) {
        self.state.borrow_mut().calls.push(format!("report: {message}"));
    }
}

fn device() -> (Device<Fake>, Rc<RefCell<State>>) {
    let state = Rc::new(RefCell::new(State {
        present: true,
        pid: Some("42\n".into()),
        cmdline: Some(b"/bin/sh\0/bin/oos-watchdog\0".to_vec()),
        request: None,
        signal_error: None,
        alive_error: None,
        busy: 0,
        open_error: None,
        remove_error: None,
        calls: vec![],
        elapsed: Duration::ZERO,
        ack: Ack { value: None, after: Some(Duration::ZERO), reply: "99\n".into(), error: None },
    }));
    (Device::new(Fake { state: state.clone(), start: Instant::now() }), state)
}

#[test]
fn preflight_checks_pid_and_exact_cmdline_without_changing_anything() {
    let (d, s) = device();
    assert_eq!(d.preflight(), Ok(Some(42)));
    for value in [None, Some(""), Some("abc"), Some("-1"), Some("0"), Some("1"), Some("2147483648")] {
        s.borrow_mut().pid = value.map(str::to_owned);
        assert!(d.preflight().unwrap_err().contains("pid"));
    }
    s.borrow_mut().pid = Some("42".into());
    for cmd in [None, Some(b"oos-watchdog".to_vec()), Some(b"/bin/oos-watchdog-old\0".to_vec())] {
        s.borrow_mut().cmdline = cmd;
        assert!(d.preflight().is_err());
    }
    assert!(s.borrow().calls.is_empty());
}

#[test]
fn absent_device_skips_pid_and_ping_but_logs_the_decision() {
    let (mut d, s) = device();
    s.borrow_mut().present = false;
    s.borrow_mut().pid = None;
    assert_eq!(d.preflight(), Ok(None));
    d.take().unwrap();
    d.ping().unwrap();
    drop(d);
    assert_eq!(s.borrow().calls.len(), 1);
    assert!(s.borrow().calls[0].contains("without a watchdog ping"));
}

#[test]
fn leftover_request_recovery_requires_a_recorded_dead_requester() {
    for error in [None, Some(libc::EPERM), Some(libc::ESRCH)] {
        let (d, s) = device();
        s.borrow_mut().request = Some("99\n".into());
        s.borrow_mut().alive_error = error;
        assert!(d.preflight().unwrap_err().contains("leftover /tmp/oos-watchdog.handoff"));
        assert!(s.borrow().calls.is_empty(), "preflight must be read-only");
        let result = d.recover_stale_request();
        assert_eq!(result.is_ok(), error == Some(libc::ESRCH));
        assert_eq!(s.borrow().request.is_none(), result.is_ok());
    }
    for requester in ["", "junk", "-1", "1"] {
        let (d, s) = device();
        s.borrow_mut().request = Some(requester.into());
        assert!(d.recover_stale_request().unwrap_err().contains("leftover"));
        assert!(s.borrow().calls.is_empty());
        assert!(s.borrow().request.is_some());
    }
}

#[test]
fn busy_device_retries_until_release_or_two_second_deadline() {
    for busy in [3, 500] {
        let (mut d, s) = device();
        s.borrow_mut().busy = busy;
        assert_eq!(d.take().is_ok(), busy == 3);
        if busy == 3 {
            d.ping().unwrap();
        }
        drop(d);
        let s = s.borrow();
        assert_eq!(s.calls.iter().filter(|&c| c == "open").count(), if busy == 3 { 4 } else { 201 });
        assert_eq!(s.elapsed, Duration::from_millis(if busy == 3 { 30 } else { 2000 }));
        assert!(s.request.is_none());
    }
}

#[test]
fn failed_signal_or_open_cleans_up_the_owned_request() {
    for signal in [true, false] {
        let (mut d, s) = device();
        if signal {
            s.borrow_mut().signal_error = Some(libc::ESRCH);
        } else {
            s.borrow_mut().open_error = Some(libc::EIO);
        }
        let result = super::jump(&mut d, || panic!("must not mark"), |_| panic!("must not reboot"));
        assert!(result.contains(if signal { "signal" } else { "takeover" }));
        drop(d);
        let s = s.borrow();
        assert!(s.request.is_none());
        assert_eq!(s.calls.iter().filter(|&c| c == "remove").count(), 1);
    }
}

#[test]
fn restore_and_drop_close_before_resuming_and_are_idempotent() {
    let (mut d, s) = device();
    d.take().unwrap();
    d.restore();
    d.restore();
    drop(d);
    assert_eq!(s.borrow().calls, ["request", "signal", "open", "close", "remove"]);
}

#[test]
fn drop_alone_restores_and_a_failed_removal_is_reported_and_retried() {
    let (mut d, s) = device();
    d.take().unwrap();
    s.borrow_mut().remove_error = Some(libc::EIO);
    d.restore();
    assert!(s.borrow().calls.last().unwrap().contains("cannot resume watchdog supervisor"));
    s.borrow_mut().remove_error = None;
    drop(d);
    assert!(s.borrow().request.is_none());
    assert_eq!(s.borrow().calls.iter().filter(|&c| c == "close").count(), 1);
    assert_eq!(s.borrow().calls.last().unwrap(), "remove");
}

#[test]
fn takeover_waits_for_matching_ack_even_when_the_device_is_already_closed() {
    for after in [Some(Duration::from_millis(30)), None] {
        let (mut d, s) = device();
        s.borrow_mut().ack.after = after;
        let result = d.take();
        assert_eq!(result.is_ok(), after.is_some());
        if after.is_none() {
            assert!(result.expect_err("must time out").contains("acknowledgement timed out"));
            assert!(!s.borrow().calls.iter().any(|c| c == "open"));
        }
        assert_eq!(s.borrow().elapsed, after.unwrap_or(Duration::from_secs(2)));
        drop(d);
        assert!(s.borrow().request.is_none());
    }
}

#[test]
fn stale_acknowledgement_refuses_takeover_before_signalling() {
    let (mut d, s) = device();
    s.borrow_mut().ack.value = Some("99\n".into());
    assert!(d.take().expect_err("stale ack").contains("previous acknowledgement"));
    assert!(s.borrow().calls.is_empty());
}

#[test]
fn wrong_or_unreadable_ack_never_opens_and_open_retries_share_its_deadline() {
    for mode in ["wrong", "unreadable", "busy"] {
        let (mut d, s) = device();
        match mode {
            "wrong" => s.borrow_mut().ack.reply = "88\n".into(),
            "unreadable" => s.borrow_mut().ack.error = Some(libc::EIO),
            _ => {
                s.borrow_mut().ack.after = Some(Duration::from_millis(1990));
                s.borrow_mut().busy = 500;
            }
        }
        assert!(d.take().is_err());
        drop(d);
        let s = s.borrow();
        assert!(s.request.is_none());
        assert_eq!(s.calls.iter().any(|c| c == "open"), mode == "busy");
        assert_eq!(s.elapsed, Duration::from_secs(if mode == "unreadable" { 0 } else { 2 }));
    }
}
