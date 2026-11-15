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
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("oos-watchdog-handoff-{}", std::process::id()));
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
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rootfs/initramfs/bin/oos-watchdog");
        let output = fs::File::create(root.join("log")).expect("log file");
        let child = Command::new("sh")
            .arg(script)
            .env("PATH", format!("{}:{}", root.join("bin").display(), std::env::var("PATH").expect("PATH set")))
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
        fs::write(self.root.join("control.handoff"), b"").expect("handoff request written");
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
    let s = Supervisor::new();
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
