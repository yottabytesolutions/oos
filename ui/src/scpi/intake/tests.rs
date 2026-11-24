// SPDX-License-Identifier: GPL-2.0-or-later
use super::*;
use crate::netport::{Budget, Id, Wake, RESPONSE_BYTES};
use std::sync::Arc;

fn ports() -> Arc<Ports> {
    Ports::new(Arc::new(Wake::new().unwrap())).unwrap()
}
fn submit(ports: &Ports, n: u64) {
    let text = format!("set {n}");
    let request = Envelope::new(Id::new(n, 1), RemoteRequest::Program(text.clone()));
    assert!(ports.requests().submit(request.id(), request, text.capacity()).is_ok());
}

#[test]
fn quota_and_injected_time_yield_without_replaying_setters() {
    let ports = ports();
    let mut intake = Intake::default();
    for n in 1..=8 {
        submit(&ports, n);
    }
    let mut calls = 0;
    intake.poll(
        &ports,
        || Duration::ZERO,
        |p, _| {
            calls += 1;
            Some(p.immediate(&mut |_| Vec::new()))
        },
    );
    // One queued reply and one parked reply fill output; frame/input remain free to advance.
    assert_eq!(calls, 2);
    for _ in 0..4 {
        intake.poll(&ports, || Duration::ZERO, |_, _| panic!("setter replay"));
    }
    assert_eq!(ports.replies().take(1)[0].id().connection(), 1);
    intake.poll(&ports, || Duration::from_micros(500), |_, _| panic!("time budget expired"));
    assert_eq!(calls, 2);
    intake.poll(
        &ports,
        || Duration::ZERO,
        |p, _| {
            calls += 1;
            Some(p.immediate(&mut |_| Vec::new()))
        },
    );
    assert_eq!(ports.replies().take(1)[0].id().connection(), 2);
    assert_eq!(calls, 3);
}

#[test]
fn a_pending_program_yields_to_the_next_connection() {
    let ports = ports();
    submit(&ports, 1);
    submit(&ports, 2);
    let mut intake = Intake::default();
    intake.poll(
        &ports,
        || Duration::ZERO,
        |p, _| {
            let mut command = String::new();
            p.immediate(&mut |text| {
                command = text.to_owned();
                Vec::new()
            });
            (command == "set 2").then(Vec::new)
        },
    );
    assert!(ports.requests().is_empty());
    assert_eq!(ports.replies().take(1)[0].id().connection(), 2);
    assert_eq!(intake.waiting.len(), 1);
    intake.poll(
        &ports,
        || Duration::ZERO,
        |p, _| {
            p.immediate(&mut |_| panic!("the suspended setter must not replay"));
            Some(Vec::new())
        },
    );
    assert_eq!(ports.replies().take(1)[0].id().connection(), 1);
}

#[test]
fn all_clients_share_one_response_budget_and_disconnect_releases_it() {
    let ports = ports();
    let reservation = ports.budget().reserve(RESPONSE_BYTES).unwrap();
    for _ in 1..8 {
        assert!(ports.budget().reserve(RESPONSE_BYTES).is_none());
    }
    submit(&ports, 1);
    let mut intake = Intake::default();
    intake.poll(&ports, || Duration::ZERO, |_, _| panic!("must reserve before dispatch"));
    drop(reservation);
    intake.poll(&ports, || Duration::ZERO, |_, _| None);
    assert_eq!(ports.budget().used(), 0, "an empty continuation releases its unused budget");
    let active = intake.waiting.front().unwrap();
    active.request.cancel();
    intake.poll(&ports, || Duration::ZERO, |_, _| panic!("disconnected program"));
    assert_eq!(ports.budget().used(), 0);
    let budget = Budget::new(Arc::new(Wake::new().unwrap()));
    let reply = Reply::new(Id::new(1, 1), Vec::with_capacity(4096), budget.reserve(RESPONSE_BYTES).unwrap());
    assert_eq!(
        budget.used(),
        4096 + std::mem::size_of::<Vec<u8>>(),
        "capacity is charged even for a zero-length payload"
    );
    drop(reply);
    assert_eq!(budget.used(), 0);
}

#[test]
fn exactly_eight_turns_and_elapsed_budget_roll_over_fifo() {
    let ports = ports();
    for n in 1..=8 {
        submit(&ports, n);
    }
    let mut intake = Intake::default();
    let mut visited = Vec::new();
    intake.poll(
        &ports,
        || Duration::ZERO,
        |p, _| {
            ports.replies().take(1);
            p.immediate(&mut |command| {
                visited.push(command.to_string());
                Vec::new()
            });
            if visited.len() == 1 {
                submit(&ports, 9);
            }
            Some(Vec::new())
        },
    );
    assert_eq!(visited, (1..=8).map(|n| format!("set {n}")).collect::<Vec<_>>());
    assert!(!ports.requests().is_empty());
    ports.replies().take(1);
    let mut clock = [Duration::ZERO, Duration::from_micros(500)].into_iter();
    intake.poll(
        &ports,
        || clock.next().unwrap_or(Duration::from_micros(500)),
        |p, _| {
            p.immediate(&mut |command| {
                visited.push(command.to_string());
                Vec::new()
            });
            Some(Vec::new())
        },
    );
    assert_eq!(visited.last().unwrap(), "set 9");
    assert_eq!(visited.len(), 9);
}

#[test]
fn idle_and_parked_intake_do_not_resignal_a_busy_loop() {
    let ports = ports();
    let mut intake = Intake::default();
    ports.requests().wake().drain();
    intake.poll(&ports, || Duration::ZERO, |_, _| panic!("idle"));
    assert!(!ports.requests().wake().wait(0));
    submit(&ports, 1);
    submit(&ports, 2);
    intake.poll(&ports, || Duration::ZERO, |_, _| Some(Vec::new()));
    ports.requests().wake().drain();
    intake.poll(&ports, || Duration::ZERO, |_, _| panic!("output full"));
    assert!(!ports.requests().wake().wait(0));
}

#[test]
fn a_full_output_port_parks_owned_bytes_while_frames_and_input_advance() {
    let ports = ports();
    let placeholder = Reply::new(Id::new(0, 0), Vec::new(), ports.budget().reserve(RESPONSE_BYTES).unwrap());
    assert!(ports.replies().submit(placeholder.id(), placeholder, 0).is_ok());
    submit(&ports, 1);
    let mut intake = Intake::default();
    let mut setters = 0;
    intake.poll(
        &ports,
        || Duration::ZERO,
        |p, _| {
            setters += 1;
            p.immediate(&mut |_| b"done".to_vec());
            Some(Vec::new())
        },
    );
    assert_eq!(setters, 1);
    assert!(ports.budget().used() > 0);
    let (done, completed) = std::sync::mpsc::channel();
    let frame_ports = ports.clone();
    let frames = std::thread::spawn(move || {
        let mut ui = crate::Ui::for_tests(crate::Source::Demo { t: 0.0 });
        for _ in 0..4 {
            crate::cmd::run(&mut ui, "workflow sim");
            ui.step(0.02);
            intake.poll(&frame_ports, || Duration::ZERO, |_, _| panic!("setter replay"));
        }
        done.send(intake).unwrap();
    });
    let mut intake = completed
        .recv_timeout(crate::ui::lifecycle::test_wait::HANG_GUARD)
        .expect("frames and input must finish before output is drained");
    frames.join().unwrap();
    ports.replies().take(1);
    intake.poll(&ports, || Duration::ZERO, |_, _| panic!("setter replay"));
    let reply = ports.replies().take(1).pop().unwrap();
    assert_eq!(reply.output().chunks().flatten().copied().collect::<Vec<_>>(), b"done\n");
    assert_eq!(ports.budget().used(), reply.capacity());
    drop(reply);
    assert_eq!(ports.budget().used(), 0);
}

#[test]
fn stalled_client_does_not_delay_another_connections_exchange() {
    let ports = ports();
    let mut intake = Intake::default();
    let now = std::cell::Cell::new(Duration::ZERO);
    let turn = Duration::from_micros(100);
    submit(&ports, 1);
    intake.poll(
        &ports,
        || now.get(),
        |program, _| {
            now.set(now.get() + turn);
            program.immediate(&mut |_| vec![b'a'; 16 << 20]);
            Some(Vec::new())
        },
    );
    // IO keeps this allocation and connection credit while its writer makes no progress.
    let stalled = ports.replies().take(1).pop().unwrap();
    assert!(ports.budget().used() >= 16 << 20);
    let accepted = now.get();
    submit(&ports, 2);
    intake.poll(
        &ports,
        || now.get() - accepted,
        |program, _| {
            now.set(now.get() + turn);
            program.immediate(&mut |_| b"ready".to_vec());
            Some(Vec::new())
        },
    );
    let healthy = ports.replies().take(1).pop().expect("healthy client must complete before stalled output drains");
    assert_eq!(healthy.id().connection(), 2);
    assert_eq!(healthy.output().chunks().flatten().copied().collect::<Vec<_>>(), b"ready\n");
    assert!(now.get() - accepted <= Duration::from_millis(33), "one frame budget, using injected time");
    assert!(ports.budget().used() <= RESPONSE_BYTES);
    drop((healthy, stalled));
    assert_eq!(ports.budget().used(), 0);
}
