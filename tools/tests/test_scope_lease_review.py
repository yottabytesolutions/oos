# Copyright (C) 2026 OOS contributors
# SPDX-License-Identifier: GPL-2.0-or-later
"""Offline regressions for the task 0021 review decisions."""
import json
import os
import signal
import threading
import time
import unittest
from unittest.mock import patch

from test_scope_lease import LeaseFixture, sl


class ReviewTests(LeaseFixture, unittest.TestCase):
    def test_tokens_required_even_with_matching_names(self):
        info = self.acquire()
        original = self.fake.info.read_bytes()
        for action in (self.client.check, self.client.renew, self.client.release):
            for token in ("", "wrong", "b" * 32):
                with self.subTest(action=action.__name__, token=token):
                    calls = self.fake.calls
                    with patch.dict(os.environ, OOS_LEASE_TOKEN=token):
                        with self.assertRaisesRegex(sl.LeaseError, "token|TOKEN"):
                            action("agent-a", "0021")
                    if token != "b" * 32:
                        self.assertEqual(self.fake.calls, calls)
                    self.assertEqual(self.fake.info.read_bytes(), original)
        with patch.dict(os.environ, OOS_LEASE_TOKEN=""):
            self.client.check("agent-a", "0021", token=info.token)
            self.client.renew("agent-a", "0021", token=info.token)
            self.client.release("agent-a", "0021", token=info.token)

    def test_cli_token_output_and_explicit_token_override(self):
        code, out, err = self.cli("acquire", "--wiring", "CH1 to reference")
        self.assertEqual((code, err), (0, ""))
        record = json.loads(out)["info"]
        token = record["token"]
        self.assertEqual(len(token), 32)
        self.assertEqual(record["wiring"], "CH1 to reference")
        with patch.dict(os.environ, OOS_LEASE_TOKEN=""):
            for command in ("status", "check", "renew"):
                args = [command]
                if command != "status":
                    self.assertEqual(self.cli(command)[0], 1)
                    args += ["--token", token]
                code, out, err = self.cli(*args)
                self.assertEqual((code, err), (0, ""))
                self.assertNotIn("token", out)
                self.assertNotIn(token, out)
            code, out, err = self.cli("status", "--cap", "observe", "--token", token)
            self.assertEqual((code, err), (0, ""))
            self.assertNotIn(token, out)
            self.assertNotIn("token", out)
            self.assertEqual(self.cli("release", "--token", token)[0], 0)

    def test_device_clock_wins_over_client_clock_skew(self):
        self.client.clock = lambda: -1000000000
        info = self.acquire()
        self.assertEqual((info.started, info.heartbeat), (1000, 1000))
        self.client.clock = lambda: 9999999999
        self.now = 1120
        self.assertEqual(self.client.renew("agent-a", "0021").heartbeat, 1120)
        self.now += sl.EXPIRY + 1
        self.client.clock = lambda: -1000000000
        for action in (self.client.check, self.client.renew, self.client.release):
            with self.subTest(action=action.__name__), self.assertRaisesRegex(sl.LeaseError, "expired"):
                action("agent-a", "0021")
        self.assertEqual(json.loads(self.cli("status")[1])["state"], "quarantined")

    def test_renew_checks_device_age_again_after_waiting_for_mutation(self):
        self.acquire()

        def advance_before_swap(script):
            if "lease changed" in script:
                self.now += sl.EXPIRY + 1
                self.fake.before = None

        self.fake.before = advance_before_swap
        with self.assertRaisesRegex(sl.LeaseError, "expired"):
            self.client.renew("agent-a", "0021")
        self.assertEqual(self.client.status().heartbeat, 1000)

    def test_device_uptime_rollback_refuses_renew_without_changing_record(self):
        self.acquire()
        original = self.fake.info.read_bytes()
        self.now -= 1
        with self.assertRaisesRegex(sl.LeaseError, "uptime moved backwards"):
            self.client.renew("agent-a", "0021")
        self.assertEqual(self.fake.info.read_bytes(), original)

    def test_wiring_is_optional_bounded_and_safely_quoted(self):
        info = self.acquire()
        self.fake.info.write_text(self.fake.info.read_text().replace('wiring=""\n', ""))
        self.assertEqual(self.client.status(), info)
        self.client.renew("agent-a", "0021")
        self.client.release("agent-a", "0021")
        wiring = 'CH1 = "reference"; CH2=\'load\'; $(false) `false` \\ Ω'
        info = self.client.acquire("agent-a", "0021", wiring=wiring)
        os.environ["OOS_LEASE_TOKEN"] = info.token
        self.assertEqual(self.client.status().wiring, wiring)
        self.assertEqual(self.client.renew("agent-a", "0021").wiring, wiring)
        self.client.release("agent-a", "0021")
        for value in ("x" * (sl.WIRING_LIMIT + 1), "CH1\nCH2", "CH1\rCH2", "\x00"):
            calls = self.fake.calls
            with self.subTest(value=value), self.assertRaisesRegex(sl.LeaseError, "wiring"):
                self.client.acquire("agent-a", "0021", wiring=value)
            self.assertEqual(self.fake.calls, calls)
        self.assertEqual(self.client.acquire("agent-a", "0021", wiring="x" * sl.WIRING_LIMIT).wiring,
                         "x" * sl.WIRING_LIMIT)

    def test_nontext_wiring_quarantines_record(self):
        self.acquire()
        raw = self.fake.info.read_text()
        self.fake.info.write_text(raw.replace('wiring=""', 'wiring=["x"]'))
        with self.assertRaisesRegex(sl.LeaseError, "quarantined"):
            self.client.status()

    def test_guard_busy_during_renew_is_retried(self):
        original = self.acquire()
        guard = self.fake.data / "lease.guard"
        guard.mkdir()
        timer = threading.Timer(0.15, guard.rmdir)
        timer.start()
        try:
            self.now += 120
            info = self.client.renew("agent-a", "0021")
        finally:
            timer.join()
        self.assertEqual(info.token, original.token)
        self.assertEqual(info.heartbeat, 1120)
        self.assertFalse(guard.exists())

    def test_busy_guard_exhausts_bounded_retry_without_removal(self):
        guard = self.fake.data / "lease.guard"
        guard.mkdir()
        started = time.monotonic()
        with self.assertRaisesRegex(sl.LeaseError, "busy"):
            self.client.status()
        self.assertLess(time.monotonic() - started, 5)
        self.assertTrue(guard.is_dir())
        self.assertFalse((self.fake.data / "lease").exists())

    def test_stranded_guard_break_is_explicit_age_limited_and_logged(self):
        self.acquire()
        original = self.fake.info.read_bytes()
        guard = self.fake.data / "lease.guard"
        guard.mkdir()
        os.utime(guard, (self.now - 60, self.now - 60))
        code, out, err = self.cli("break", "--guard", "--reason", "owner checked")
        self.assertEqual((code, out), (1, ""))
        self.assertIn("older than 60", err)
        self.assertTrue(guard.exists())
        self.assertFalse((self.fake.data / "lease.log").exists())
        self.now += 1
        reason = 'Owner inspected; "quotes" $(false) `false`'
        code, out, err = self.cli("break", "--guard", "--reason", reason)
        self.assertEqual((code, err), (0, ""))
        self.assertIn("guard broken", out)
        self.assertFalse(guard.exists())
        self.assertEqual(self.fake.info.read_bytes(), original)
        entry = json.loads((self.fake.data / "lease.log").read_text())
        self.assertEqual(entry, dict(reason=reason, holder="agent-a", task="0021",
                                     at=1001, guard_mtime=940, guard=True))

    def test_guard_break_needs_reason_and_successful_logging(self):
        guard = self.fake.data / "lease.guard"
        guard.mkdir()
        os.utime(guard, (900, 900))
        with self.assertRaisesRegex(sl.LeaseError, "reason"):
            self.client.break_lease("", "owner", "0021", guard=True)
        (self.fake.data / "lease.log").mkdir()
        with self.assertRaises(sl.LeaseError):
            self.client.break_lease("Owner inspected", "owner", "0021", guard=True)
        self.assertTrue(guard.exists())

    def test_heartbeat_retries_transient_failure_then_recovers(self):
        self.acquire()
        failures, recovered = [], threading.Event()

        def executor(host, script):
            if threading.current_thread() is not threading.main_thread():
                if not failures:
                    failures.append("busy")
                    raise sl.LeaseError("lease operation busy")
                result = self.fake(host, script)
                if "mv " in script:
                    recovered.set()
                return result
            return self.fake(host, script)

        with sl.held(executor=executor, clock=lambda: self.now, interval=0.01) as lease:
            self.assertTrue(recovered.wait(3))
            self.assertIsNone(lease.error)
            lease.check()
        self.assertEqual(failures, ["busy"])

    def test_heartbeat_stops_at_expiry_margin_and_does_not_call_after_exit(self):
        self.acquire()
        failed = threading.Event()
        attempts = []

        def executor(host, script):
            if threading.current_thread() is not threading.main_thread():
                attempts.append(self.now)
                failed.set()
                raise sl.LeaseError("connection lost")
            return self.fake(host, script)

        with self.assertRaisesRegex(sl.LeaseError, "renewal failed"):
            with sl.held(executor=executor, clock=lambda: self.now, interval=0.01) as lease:
                self.assertTrue(failed.wait(3))
                self.now = 1000 + sl.EXPIRY - sl.RENEW_MARGIN - 1
                lease.check()
                self.now += 1
                with self.assertRaisesRegex(sl.LeaseError, "renewal failed"):
                    lease.check()
        count = len(attempts)
        time.sleep(0.05)
        self.assertEqual(len(attempts), count)
        self.assertEqual(count, 1)

    def test_no_executor_calls_after_successful_held_exit(self):
        self.acquire()
        calls = []

        def executor(host, script):
            calls.append(script)
            return self.fake(host, script)

        with sl.held(executor=executor, interval=0.01):
            pass
        count = len(calls)
        time.sleep(0.05)
        self.assertEqual(len(calls), count)

    def test_late_renewal_cannot_resume_after_expiry_margin(self):
        self.acquire()
        completed = threading.Event()

        def executor(host, script):
            result = self.fake(host, script)
            if threading.current_thread() is not threading.main_thread() and "mv " in script:
                self.now = 1000 + sl.EXPIRY - sl.RENEW_MARGIN
                completed.set()
            return result

        with self.assertRaisesRegex(sl.LeaseError, "renewal failed"):
            with sl.held(executor=executor, clock=lambda: self.now, interval=0.01) as lease:
                self.assertTrue(completed.wait(3))
                with self.assertRaisesRegex(sl.LeaseError, "renewal failed"):
                    lease.check()

    def test_exit_joins_inflight_renewal_before_returning(self):
        self.acquire()
        started, proceed, completed = threading.Event(), threading.Event(), threading.Event()
        calls = []

        def executor(host, script):
            calls.append(script)
            if threading.current_thread() is not threading.main_thread():
                started.set()
                if not proceed.wait(3):
                    raise sl.LeaseError("test worker timed out")
            result = self.fake(host, script)
            if threading.current_thread() is not threading.main_thread() and "mv " in script:
                completed.set()
            return result

        timer = threading.Timer(0.05, proceed.set)
        try:
            with sl.held(executor=executor, interval=0.01):
                self.assertTrue(started.wait(3))
                timer.start()
            self.assertTrue(completed.is_set())
            count = len(calls)
            time.sleep(0.05)
            self.assertEqual(len(calls), count)
        finally:
            proceed.set()
            if timer.ident is not None:
                timer.join()

    def test_guard_cleanup_traps_pipe_and_other_catchable_signals(self):
        self.assert_guard_signal_cleanup()

    def assert_guard_signal_cleanup(self):
        for name in ("PIPE", "HUP", "INT", "TERM"):
            with self.subTest(signal=name):
                with self.assertRaises(sl.LeaseError):
                    # Stay alive until the trap exits; EOF can race signal delivery.
                    # FakeSSH's timeout fails the test if the signal is ignored.
                    self.client._run(f"kill -s {name} $$\nwhile :; do :; done")
                self.assertFalse((self.fake.data / "lease.guard").exists())

    def test_guard_cleanup_with_inherited_sigint_ignored(self):
        previous = signal.signal(signal.SIGINT, signal.SIG_IGN)
        try:
            self.assert_guard_signal_cleanup()
        finally:
            signal.signal(signal.SIGINT, previous)

    def test_guard_cleanup_when_signalled_during_acquisition(self):
        for name in ("PIPE", "HUP", "INT", "TERM"):
            for busy in (False, True):
                with self.subTest(signal=name, busy=busy):
                    self.assert_acquisition_signal_cleanup(name, busy)

    def assert_acquisition_signal_cleanup(self, name, busy):
        guard = self.fake.data / "lease.guard"
        if busy:
            guard.mkdir()

        def executor(host, script):
            # Signal before mkdir's caller regains control, with its real exit status.
            prefix = f'''mkdir() {{
    status=0
    command mkdir "$@" || status=$?
    kill -s {name} $$
    # The shell dispatches pending signals after a builtin returns.
    :
    return "$status"
}}
'''
            return self.fake(host, prefix + script)

        try:
            with patch.object(self.client, "executor", executor):
                with self.assertRaises(sl.LeaseError):
                    self.client._run(":")
            self.assertEqual(guard.exists(), busy)
        finally:
            # Keep subtests independent even when a regression strands the guard.
            if guard.exists():
                guard.rmdir()
