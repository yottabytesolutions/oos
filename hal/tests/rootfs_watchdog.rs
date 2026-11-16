// Copyright (C) 2026 Julian Decker
// SPDX-License-Identifier: GPL-2.0-or-later

//! Exercise the real supervisor handshake on the host; the fake kicker logs a magic close on TERM, never on KILL.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Supervisor {
    root: PathBuf,
    child: Child,
}

impl Supervisor {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!("oos-watchdog-handoff-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("bin")).expect("fixture bin dir");
        let fake = root.join("bin/watchdog");
        fs::write(
            &fake,
            concat!(
                "#!/usr/bin/env python3\n",
                "import os, signal\n",
                "def magic(*args):\n    print('MAGIC CLOSE', flush=True)\n    raise SystemExit(0)\n",
                "signal.signal(signal.SIGTERM, magic)\n",
                "print('kicker ready %s' % os.getpid(), flush=True)\n",
                "while True: signal.pause()\n",
            ),
        )
        .expect("fake kicker written");
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).expect("fake kicker executable");
        let sleep = root.join("bin/sleep");
        fs::write(&sleep, "#!/bin/sh\nif [ \"$1\" = 1 ]; then echo backoff > \"$OOS_WATCHDOG_CONTROL.sleep\"; exec /bin/sleep 8; fi\nexec /bin/sleep \"$@\"\n").expect("sleep shim");
        fs::set_permissions(sleep, fs::Permissions::from_mode(0o755)).expect("sleep executable");
        let script = supervisor_script(&root, tag);
        let output = fs::File::create(root.join("log")).expect("log file");
        let child = Command::new(std::env::var("OOS_TEST_SHELL").unwrap_or_else(|_| "sh".into()))
            .arg(script)
            .env("PATH", format!("{}:{}", root.join("bin").display(), std::env::var("PATH").expect("PATH set")))
            .env("OOS_REQUESTER", std::process::id().to_string())
            .env("OOS_WATCHDOG_DEV", "/dev/null")
            .env("OOS_KMSG", root.join("kmsg"))
            .env("OOS_DEBUGFS", root.join("debug"))
            .env("OOS_CMDLINE", "/dev/null")
            .env("OOS_WATCHDOG_CONTROL", root.join("control"))
            .process_group(0)
            .stdin(Stdio::null())
            .stdout(output)
            .stderr(Stdio::null())
            .spawn()
            .expect("supervisor starts");
        let s = Self { root, child };
        s.wait_for("kicker ready", 1);
        s
    }

    fn log(&self) -> String {
        fs::read_to_string(self.root.join("log")).expect("log readable")
    }

    fn wait_for(&self, text: &str, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            if self.log().matches(text).count() >= count {
                return;
            }
            assert!(Instant::now() < deadline, "waiting for {text}: {}", self.log());
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn request(&self) {
        fs::write(self.root.join("control.handoff"), format!("{}\n", std::process::id()))
            .expect("handoff request written");
        let pid = fs::read_to_string(self.root.join("control.pid")).expect("pid file readable");
        assert_eq!(pid.trim(), self.child.id().to_string());
        // SAFETY: this is the test's own live supervisor child.
        assert_eq!(unsafe { libc::kill(self.child.id() as libc::pid_t, libc::SIGUSR1) }, 0);
    }

    fn resume(&self) {
        fs::remove_file(self.root.join("control.handoff")).expect("handoff request removed");
    }
}

impl Drop for Supervisor {
    fn drop(&mut self) {
        // SAFETY: spawn placed this test's supervisor and fake kickers in their own process group.
        unsafe { libc::kill(-(self.child.id() as libc::pid_t), libc::SIGKILL) };
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn handoff_reaps_without_magic_close_and_resumes_after_each_failed_jump() {
    let s = Supervisor::new("handoff");
    for attempt in 1..=2 {
        s.request();
        s.wait_for("kexec handoff, kicker reaped", attempt);
        // Holding the request models the updater's open handle (or its death). No competing reopen or magic close.
        std::thread::sleep(Duration::from_millis(250));
        assert_eq!(s.log().matches("kicker ready").count(), attempt);
        assert!(!s.log().contains("MAGIC CLOSE"));
        s.resume();
        s.wait_for("kicker ready", attempt + 1);
        let log = s.log();
        assert_eq!(log.matches("kexec returned, resuming kicker").count(), attempt);
        assert!(!log.contains("starting it again in"), "handoff must bypass crash backoff: {log}");
    }
    // Cancellation can race the supervisor's trap. It must still reap and reopen without losing the live child.
    s.request();
    s.resume();
    s.wait_for("kicker ready", 4);
    assert_eq!(s.log().matches("kexec handoff, kicker reaped").count(), 3);
    assert!(!s.log().contains("MAGIC CLOSE"));
}

fn crash(s: &Supervisor) {
    let line = s.log().lines().find(|l| l.starts_with("kicker ready ")).expect("fixture value").to_string();
    let pid: i32 = line.split_whitespace().last().expect("fixture value").parse().expect("fixture value");
    // SAFETY: this is the fixture's fake kicker, whose pid was read from its ready line.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGKILL) }, 0);
}

fn backoff(s: &Supervisor) {
    crash(s);
    let deadline = Instant::now() + Duration::from_secs(3);
    while !s.root.join("control.sleep").exists() {
        assert!(Instant::now() < deadline, "backoff did not start: {}", s.log());
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn acknowledged(s: &Supervisor) {
    let deadline = Instant::now() + Duration::from_secs(2);
    let expected = format!("{}\n", std::process::id());
    while fs::read_to_string(s.root.join("control.ack")).ok().as_ref() != Some(&expected) {
        assert!(Instant::now() < deadline, "handoff not acknowledged during backoff: {}", s.log());
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn usr1_during_backoff_acknowledges_before_the_sleep_finishes() {
    let s = Supervisor::new("backoff");
    backoff(&s);
    s.request();
    acknowledged(&s);
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(s.log().matches("kicker ready").count(), 1);
    assert!(!s.log().contains("MAGIC CLOSE"));
}

#[test]
fn kexec_failure_during_backoff_resumes_without_waiting_for_the_sleep() {
    let s = Supervisor::new("undo-backoff");
    backoff(&s);
    s.request();
    acknowledged(&s);
    s.resume(); // returned reboot(KEXEC): close handle, then cancel request
    let deadline = Instant::now() + Duration::from_secs(2);
    while s.log().matches("kicker ready").count() < 2 || s.root.join("control.ack").exists() {
        assert!(Instant::now() < deadline, "kexec undo stranded the kicker in backoff: {}", s.log());
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(s.log().contains("kexec returned, resuming kicker"));
    assert!(!s.log().contains("MAGIC CLOSE"));
}

// Inject only scheduling boundaries; execute the production loop and trap unchanged.
fn supervisor_script(root: &std::path::Path, tag: &str) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rootfs/initramfs/bin/oos-watchdog");
    let mut script = fs::read_to_string(source).expect("supervisor source");
    if tag == "before-wait" {
        script.insert_str(
            0,
            concat!(
                "wait() {\n",
                "  if [ -n \"${sleeper:-}\" ] && [ \"$1\" = \"$sleeper\" ] && [ ! -e \"$control.injected\" ]; then\n",
                "    : > \"$control.injected\"\n",
                "    echo \"$OOS_REQUESTER\" > \"$control.handoff\"\n",
                "    kill -USR1 \"$$\"\n",
                "  fi\n",
                "  command wait \"$@\"\n}\n",
            ),
        );
    }
    if tag == "during-log" || tag == "ack-removal" {
        let (name, action) = if tag == "during-log" {
            (
                "tee",
                "line = sys.stdin.read()\nprint(line, end='', flush=True)\ntrigger = 'starting it again in' in line",
            )
        } else {
            ("rm", "trigger = str(control) + '.ack' in sys.argv\nfor arg in sys.argv[1:]:\n if arg != '-f': pathlib.Path(arg).unlink(missing_ok=True)")
        };
        let shim = format!(
            "#!/usr/bin/env python3\nimport os, pathlib, signal, sys\ncontrol = pathlib.Path(os.environ['OOS_WATCHDOG_CONTROL'])\n{action}\nmark = pathlib.Path(str(control) + '.injected')\nif trigger and not mark.exists():\n mark.touch()\n pathlib.Path(str(control) + '.handoff').write_text(os.environ['OOS_REQUESTER'] + '\\n')\n os.kill(int(pathlib.Path(str(control) + '.pid').read_text()), signal.SIGUSR1)\n"
        );
        let path = root.join("bin").join(name);
        fs::write(&path, shim).expect("signal shim");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("signal shim executable");
    }
    let path = root.join("supervisor");
    fs::write(&path, script).expect("supervisor fixture");
    path
}

#[test]
fn usr1_between_backoff_check_and_wait_is_acknowledged_promptly() {
    let s = Supervisor::new("before-wait");
    crash(&s);
    acknowledged(&s);
    assert!(s.root.join("control.injected").exists());
    s.resume();
    s.wait_for("kicker ready", 2);
}

#[test]
fn usr1_during_backoff_logging_is_acknowledged_promptly() {
    let s = Supervisor::new("during-log");
    crash(&s);
    acknowledged(&s);
    assert!(s.root.join("control.injected").exists());
    s.resume();
    s.wait_for("kicker ready", 2);
}

#[test]
fn a_second_request_during_ack_removal_is_not_lost() {
    let s = Supervisor::new("ack-removal");
    s.request();
    acknowledged(&s);
    s.resume();
    s.wait_for("kexec handoff, kicker reaped", 2);
    acknowledged(&s);
    assert!(s.root.join("control.injected").exists());
    s.resume();
    s.wait_for("kicker ready", 2);
}
