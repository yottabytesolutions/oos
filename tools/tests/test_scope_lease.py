# Copyright (C) 2026 OOS contributors
# SPDX-License-Identifier: GPL-2.0-or-later
"""Offline lease tests. Fake SSH runs the real shell protocol in a private directory."""
from concurrent.futures import ThreadPoolExecutor
from contextlib import redirect_stderr, redirect_stdout
from dataclasses import asdict, replace
import io
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import scope_lease as sl


class FakeSSH:
    """Use mkdir's real atomic semantics, without SSH or any device access."""

    def __init__(self, root: Path):
        self.root = root
        self.data = root / "data"
        self.data.mkdir()
        self.build = root / "image-build"
        self.build.write_text("build-test kernel-test\n")
        self.calls = 0
        self.before = None
        self.clock = lambda: 1000
        self.wall_clock = lambda: self.clock()
        self.boot_id = "boot-one"

    def __call__(self, host: str, script: str) -> str:
        if host != "scope.example":
            raise AssertionError("unexpected host")
        self.calls += 1
        if self.before:
            self.before(script)
        paths = {"/etc/oos-build": self.build, "/oos": self.data}
        local = re.sub(r"/etc/oos-build|/oos", lambda m: shlex.quote(str(paths[m.group()])), script)
        local = local.replace("date +%s", f"printf '%s' {int(self.wall_clock())}")
        local = local.replace("read -r uptime rest < /proc/uptime",
                              f"uptime={int(self.clock())}.25")
        local = local.replace("cat /proc/sys/kernel/random/boot_id",
                              f"printf '%s' {shlex.quote(self.boot_id)}")
        stat = f"{shlex.quote(sys.executable)} -I -c 'import os, sys; print(int(os.stat(sys.argv[1]).st_mtime))'"
        local = local.replace("stat -c %Y", stat)
        # A background runner may inherit ignored SIGINT. Non-interactive shells
        # cannot trap it then. Reset in a fresh child, without thread-unsafe preexec_fn.
        bootstrap = ("import os, signal\n"
                     "for name in ('SIGPIPE', 'SIGHUP', 'SIGINT', 'SIGTERM'):\n"
                     "    signal.signal(getattr(signal, name), signal.SIG_DFL)\n"
                     "os.execlp('sh', 'sh', '-s')\n")
        result = subprocess.run([sys.executable, "-I", "-c", bootstrap], input=local, text=True,
                                capture_output=True, timeout=5, check=False)
        if result.returncode:
            raise sl.LeaseError(result.stderr.strip())
        return result.stdout

    @property
    def info(self) -> Path:
        return self.data / "lease" / "info"


class LeaseFixture:
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.fake = FakeSSH(Path(self.temp.name))
        self.now = 1000
        self.fake.clock = lambda: self.now
        self.client = sl.Client("scope.example", executor=self.fake, clock=lambda: self.now)
        self.env = patch.dict(os.environ, {"OOS_SCOPE": "scope.example",
                              "OOS_LEASE_HOLDER": "agent-a", "OOS_LEASE_TASK": "0021",
                              "OOS_LEASE_TOKEN": "a" * 32}, clear=True)
        self.env.start()
        self.addCleanup(self.env.stop)

    def acquire(self, cap="test", holder="agent-a", task="0021", go=""):
        info = self.client.acquire(holder, task, cap, go)
        os.environ["OOS_LEASE_TOKEN"] = info.token
        return info

    def cli(self, *args):
        stdout, stderr = io.StringIO(), io.StringIO()
        with redirect_stdout(stdout), redirect_stderr(stderr):
            code = sl.main(list(args), executor=self.fake, clock=lambda: self.now)
        return code, stdout.getvalue(), stderr.getvalue()


class LeaseTests(LeaseFixture, unittest.TestCase):
    def test_renew_does_not_publish_when_staged_file_sync_fails(self):
        info = self.acquire()
        before = self.fake.info.read_bytes()
        sync = self.fake.root / "sync"
        sync.write_text('#!/bin/sh\ncase "$1" in *.new) echo "staging sync failed" >&2; exit 1;; esac\n')
        sync.chmod(0o700)
        self.now += 1
        with patch.dict(os.environ, {"PATH": f"{self.fake.root}{os.pathsep}{os.defpath}"}):
            with self.assertRaisesRegex(sl.LeaseError, "staging sync failed"):
                self.client.renew(info.holder, info.task, token=info.token)
        self.assertEqual(self.fake.info.read_bytes(), before)

    def test_record_contains_build_and_hashed_machine(self):
        with patch.object(sl.socket, "gethostname", return_value="workstation-placeholder"):
            info = self.acquire()
        self.assertEqual(info.build_id, "build-test kernel-test")
        self.assertEqual((info.started, info.heartbeat), (1000, 1000))
        self.assertEqual(len(info.machine_hash), 64)
        self.assertNotIn("workstation-placeholder", self.fake.info.read_text())
        self.assertEqual(self.client.status(), info)

    def test_atomic_acquire_race_has_one_winner(self):
        barrier = threading.Barrier(2)
        self.fake.before = lambda script: barrier.wait(timeout=5)

        def attempt(holder):
            try:
                return self.acquire(holder=holder).holder
            except sl.LeaseError:
                return None

        with ThreadPoolExecutor(max_workers=2) as pool:
            results = list(pool.map(attempt, ("agent-a", "agent-b")))
        self.fake.before = None
        winners = [result for result in results if result]
        self.assertEqual(len(winners), 1)
        self.assertEqual(self.client.status().holder, winners[0])

    def test_renew_changes_only_heartbeat(self):
        original = self.acquire()
        self.now += 120
        renewed = self.client.renew("agent-a", "0021")
        self.assertEqual(renewed, replace(original, heartbeat=1120))
        self.assertEqual(self.client.status(), renewed)

    def test_wrong_holder_or_task_cannot_renew_or_release(self):
        self.acquire()
        original = self.fake.info.read_bytes()
        for method in (self.client.renew, self.client.release):
            for holder, task in (("agent-b", "0021"), ("agent-a", "0022")):
                with self.subTest(method=method.__name__, holder=holder, task=task):
                    with self.assertRaises(sl.LeaseError):
                        method(holder, task)
                    self.assertEqual(self.fake.info.read_bytes(), original)
        self.client.release("agent-a", "0021")
        self.assertIsNone(self.client.status())

    def test_expiry_boundary_and_quarantine_never_transfer(self):
        original = self.acquire()
        self.now = 1600
        self.assertEqual(self.client.check("agent-a", "0021"), original)
        self.now += 1
        for action in (lambda: self.acquire(holder="agent-b"),
                       lambda: self.client.renew("agent-a", "0021"),
                       lambda: self.client.release("agent-a", "0021"),
                       lambda: self.client.check("agent-a", "0021")):
            with self.subTest(action=action), self.assertRaises(sl.LeaseError):
                action()
        self.assertEqual(self.client.status(), original)
        code, out, err = self.cli("status")
        self.assertEqual((code, err), (0, ""))
        record = json.loads(out)
        self.assertEqual(record["state"], "quarantined")
        public = asdict(original)
        public.pop("token")
        self.assertEqual(record["info"], public)

    def test_break_logs_reason_and_full_old_info_before_removal(self):
        self.acquire()
        old = self.fake.info.read_text()
        reason = "owner inspected; 'quotes'\n$(false) `false`"
        self.client.break_lease(reason, "owner", "0021")
        entries = [json.loads(line) for line in (self.fake.data / "lease.log").read_text().splitlines()]
        self.assertEqual(entries[0], dict(reason=reason, old_info=old, holder="owner", task="0021", at=1000))
        self.assertIsNone(self.client.status())
        self.acquire(holder="agent-b")
        self.client.break_lease("second inspection", "owner", "0021")
        self.assertEqual(len((self.fake.data / "lease.log").read_text().splitlines()), 2)

    def test_break_requires_reason_and_logging_failure_preserves_lease(self):
        self.acquire()
        for reason in ("", " \n "):
            with self.subTest(reason=reason), self.assertRaisesRegex(sl.LeaseError, "reason"):
                self.client.break_lease(reason, "owner", "0021")
        (self.fake.data / "lease.log").mkdir()
        with self.assertRaises(sl.LeaseError):
            self.client.break_lease("owner checked", "owner", "0021")
        self.assertTrue(self.fake.info.exists())

    def test_capability_ordering_table(self):
        for cap in sl.CAPABILITIES:
            self.acquire(cap=cap, go="0021" if cap in ("install", "reboot") else "")
            for needed in sl.CAPABILITIES:
                with self.subTest(cap=cap, needed=needed):
                    if sl.capability(cap) >= sl.capability(needed):
                        self.assertEqual(self.client.check("agent-a", "0021", needed).capability, cap)
                    else:
                        with self.assertRaisesRegex(sl.LeaseError, "capability"):
                            self.client.check("agent-a", "0021", needed)
            self.client.release("agent-a", "0021")

    def test_go_required_for_install_and_reboot(self):
        for cap in ("install", "reboot"):
            for go in ("", "0022", "0021"):
                with self.subTest(cap=cap, go=go):
                    if go == "0021":
                        self.assertEqual(self.acquire(cap=cap, go=go).go, go)
                        self.client.release("agent-a", "0021")
                    else:
                        calls = self.fake.calls
                        with self.assertRaisesRegex(sl.LeaseError, "--go"):
                            self.acquire(cap=cap, go=go)
                        self.assertEqual(self.fake.calls, calls)
                        self.assertFalse(self.fake.info.parent.exists())

    def test_no_default_host_and_explicit_host_precedence(self):
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(sl.LeaseError, "--host or set OOS_SCOPE"):
                sl.Client(executor=self.fake)
            code, out, err = self.cli("status")
        self.assertEqual((code, out), (1, ""))
        self.assertIn("no default scope", err)
        self.assertEqual(self.fake.calls, 0)
        self.assertEqual(sl.host_value("192.0.2.1"), "192.0.2.1")
        self.assertEqual(sl.host_value(), "scope.example")
        for host in ("", "-bad", "root@scope.example", "scope.example;false", "scope.example\n"):
            with self.subTest(host=host), self.assertRaises(sl.LeaseError):
                sl.host_value(host)

    def test_missing_lease_or_identity_denies_without_device_access(self):
        with self.assertRaisesRegex(sl.LeaseError, "no lease"):
            with sl.held(executor=self.fake):
                self.fail("access granted")
        self.acquire()
        for key in ("OOS_LEASE_HOLDER", "OOS_LEASE_TASK"):
            with patch.dict(os.environ, {key: ""}):
                calls = self.fake.calls
                with self.assertRaises(sl.LeaseError):
                    with sl.held(executor=self.fake):
                        self.fail("access granted")
                self.assertEqual(self.fake.calls, calls)

    def test_incomplete_or_malformed_lease_is_quarantined_but_breakable(self):
        for raw in ("", "holder=agent-a\n", "not a record\n"):
            with self.subTest(raw=raw):
                self.fake.info.parent.mkdir(exist_ok=True)
                self.fake.info.write_text(raw)
                with self.assertRaisesRegex(sl.LeaseError, "quarantined"):
                    self.client.status()
                with self.assertRaises(sl.LeaseError):
                    self.acquire()
                self.client.break_lease("owner inspected incomplete record", "owner", "0021")
                self.assertIsNone(self.client.status())

    def test_failed_initialization_leaves_quarantine(self):
        self.fake.build.unlink()
        with self.assertRaisesRegex(sl.LeaseError, "quarantined"):
            self.acquire()
        with self.assertRaisesRegex(sl.LeaseError, "quarantined"):
            self.client.status()
        with self.assertRaises(sl.LeaseError):
            self.acquire(holder="agent-b")
        self.client.break_lease("owner checked build file", "owner", "0021")

    def test_stale_mutations_cannot_change_replacement(self):
        actions = (lambda: self.client.renew("agent-a", "0021"),
                   lambda: self.client.release("agent-a", "0021"),
                   lambda: self.client.break_lease("owner inspected", "owner", "0021"))
        for action in actions:
            original = self.acquire()
            replacement = []

            def replace_before_mutation(script):
                if "lease changed" in script:
                    self.fake.before = None
                    self.client.release("agent-a", "0021")
                    replacement.append(self.acquire())

            self.fake.before = replace_before_mutation
            with self.assertRaisesRegex(sl.LeaseError, "lease changed"):
                action()
            self.assertNotEqual(original.token, replacement[0].token)
            self.assertEqual(self.client.status(), replacement[0])
            self.client.release("agent-a", "0021")

    def test_held_renews_in_background_and_keeps_lease_for_explicit_release(self):
        self.acquire()
        renewed = threading.Event()
        original = self.fake

        def executor(host, script):
            result = original(host, script)
            if threading.current_thread() is not threading.main_thread() and "mv " in script:
                renewed.set()
            return result

        with sl.held(executor=executor, clock=lambda: self.now, interval=0.01) as lease:
            self.now += 120
            self.assertTrue(renewed.wait(3))
            lease.check()
        self.assertEqual(self.client.status().heartbeat, 1120)

    def test_held_raises_after_renewal_failure(self):
        self.acquire()
        failed = threading.Event()

        def executor(host, script):
            if threading.current_thread() is not threading.main_thread():
                failed.set()
                raise sl.LeaseError("connection lost")
            return self.fake(host, script)

        with self.assertRaisesRegex(sl.LeaseError, "renewal failed"):
            with sl.held(executor=executor, clock=lambda: self.now, interval=0.01) as lease:
                self.assertTrue(failed.wait(3))
                self.now += sl.EXPIRY
                # Wait for the worker to publish the error, without contacting a host.
                deadline = time.monotonic() + 3
                while lease.error is None and time.monotonic() < deadline:
                    time.sleep(0.001)
                with self.assertRaisesRegex(sl.LeaseError, "renewal failed"):
                    lease.check()

    def test_cli_checks_identity_and_capability(self):
        self.acquire()
        cases = [(('status', '--cap', 'test'), 0),
                 (('status', '--cap', 'install'), 1),
                 (('status', '--cap', 'observe', '--holder', 'agent-b'), 1),
                 (('break', '--reason', ''), 1),
                 (('renew',), 0), (('release',), 0), (('status', '--cap', 'observe'), 1)]
        for args, expected in cases:
            with self.subTest(args=args):
                self.assertEqual(self.cli(*args)[0], expected)




if __name__ == "__main__":
    unittest.main()
