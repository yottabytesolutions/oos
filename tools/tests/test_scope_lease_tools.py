# Copyright (C) 2026 OOS contributors
# SPDX-License-Identifier: GPL-2.0-or-later
"""Tool integration tests with local stubs for all transports."""
import os
from pathlib import Path
import runpy
import shlex
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

from test_scope_lease import TOOLS, sl


class ToolIntegrationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.marker = self.root / "unexpected-access"
        # Every possible transport is a local fail-fast stub, even on regressions.
        for command in ("ssh", "adb", "nc"):
            stub = self.bin / command
            stub.write_text(f"#!/bin/sh\nprintf access >> {shlex.quote(str(self.marker))}\nexit 91\n")
            stub.chmod(0o700)
        self.env = {"PATH": str(self.bin) + os.pathsep + os.environ["PATH"]}

    def python_tool(self, name, *args):
        bootstrap = ("import runpy, socket, sys, urllib.request\n"
                     "def refuse(*args, **kwargs): raise RuntimeError('network forbidden in tests')\n"
                     "socket.create_connection = refuse\n"
                     "urllib.request.urlopen = refuse\n"
                     "sys.argv = sys.argv[1:]\n"
                     "runpy.run_path(sys.argv[0], run_name='__main__')\n")
        return [sys.executable, "-I", "-c", bootstrap, str(TOOLS / name), *args]

    def test_cli_and_tools_refuse_missing_host_without_transport(self):
        commands = [self.python_tool("scope-lease", "status"), self.python_tool("device-test.py")]
        commands += [["sh", str(TOOLS / name)] for name in
                     ("device-deploy.sh", "device-survey.sh", "platform-probe.sh")]
        for command in commands:
            with self.subTest(tool=Path(command[-1]).name):
                result = subprocess.run(command, env=self.env, text=True, capture_output=True, timeout=5)
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn("no default scope", result.stderr)
                self.assertFalse(self.marker.exists())

    def test_tools_refuse_missing_identity_before_transport(self):
        commands = [self.python_tool("device-test.py")]
        commands += [["sh", str(TOOLS / name)] for name in
                     ("device-deploy.sh",)]
        for command in commands:
            with self.subTest(tool=Path(command[-1]).name):
                result = subprocess.run(command + ["--host", "scope.example"], env=self.env,
                                        text=True, capture_output=True, timeout=5)
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn("OOS_LEASE_HOLDER", result.stderr)
                self.assertFalse(self.marker.exists())

    def test_stock_tools_use_only_adb_without_lease_identity(self):
        scripts = self.root / "tools"
        scripts.mkdir()
        for name in ("device-survey.sh", "platform-probe.sh"):
            with self.subTest(tool=name):
                script = scripts / name
                script.write_text((TOOLS / name).read_text())
                self.marker.unlink(missing_ok=True)
                (self.bin / "adb").write_text(
                    f"#!/bin/sh\nprintf adb >> {shlex.quote(str(self.marker))}\nexit 91\n")
                result = subprocess.run(["sh", str(script), "--host", "scope.example"],
                                        env=self.env, text=True, capture_output=True, timeout=5)
                self.assertEqual(result.returncode, 91, result.stderr)
                self.assertEqual(self.marker.read_text(), "adb")
                if name == "device-survey.sh":
                    self.assertIn("owner's release", result.stderr)
                    self.assertIn("tools/README.md", result.stderr)

    def test_shell_tools_check_capability_and_identity_before_other_transport(self):
        env = dict(self.env, OOS_SCOPE="scope.example", OOS_LEASE_HOLDER="agent-a",
                   OOS_LEASE_TASK="0021", OOS_LEASE_TOKEN="a" * 32)
        now = int(time.time())
        cases = [("agent-a", "0021", "test", "capability"),
                 ("agent-b", "0021", "install", "belongs to"),
                 ("agent-a", "0022", "install", "belongs to")]
        for holder, task, cap, message in cases:
            with self.subTest(holder=holder, task=task, cap=cap):
                record = sl.Info(holder, task, cap, "0" * 64, "build-test", now, now,
                                 task if cap == "install" else "", "a" * 32, "boot-one")
                (self.bin / "ssh").write_text(f"#!/bin/sh\ncat <<'RECORD'\n{now} boot-one\npresent\n" +
                                              record.encode() + "RECORD\n")
                result = subprocess.run(["sh", str(TOOLS / "device-deploy.sh")], env=env,
                                        text=True, capture_output=True, timeout=5)
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn(message, result.stderr)
                self.assertFalse(self.marker.exists())

    def test_scope_refuses_absent_or_failed_lease_before_io(self):
        module = runpy.run_path(str(TOOLS / "device-test.py"))
        Scope = module["Scope"]
        with patch.object(module["socket"], "create_connection") as connect:
            with self.assertRaisesRegex(sl.LeaseError, "requires a held"):
                Scope("scope.example")
            lease = sl.HeldLease(sl.Client("scope.example"), None, sl.LeaseError("lost lease"))
            with self.assertRaisesRegex(sl.LeaseError, "lost lease"):
                Scope("scope.example", lease=lease)
            connect.assert_not_called()

    def test_simulator_bypasses_device_lease(self):
        module = runpy.run_path(str(TOOLS / "device-test.py"))
        with patch.object(module["socket"], "create_connection") as connect:
            scope = module["Scope"]("scope.example", sim=True)
            scope.check_lease()
            connect.assert_called_once()

