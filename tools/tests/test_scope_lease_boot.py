# Copyright (C) 2026 OOS contributors
# SPDX-License-Identifier: GPL-2.0-or-later
"""Offline boot and wall clock regressions for persistent leases."""
from dataclasses import replace
import json
import unittest

from test_scope_lease import LeaseFixture, sl


class BootTests(LeaseFixture, unittest.TestCase):
    def reboot(self):
        self.fake.boot_id = "boot-two"
        self.now = 5

    def test_reboot_holder_with_install_or_reboot_capability_rebinds(self):
        for cap in ("install", "reboot"):
            for action in ("check", "renew", "release"):
                with self.subTest(cap=cap, action=action):
                    self.fake.boot_id = "boot-one"
                    self.now = 1000
                    original = self.acquire(cap=cap, go="0021")
                    self.reboot()
                    self.assertEqual(json.loads(self.cli("status")[1])["state"], "quarantined")
                    rebound = getattr(self.client, action)("agent-a", "0021", token=original.token)
                    if action != "release":
                        self.assertEqual(rebound, replace(original, boot_id="boot-two", heartbeat=5))
                        self.client.release("agent-a", "0021")
                    entry = json.loads((self.fake.data / "lease.log").read_text().splitlines()[-1])
                    self.assertEqual(entry, dict(event="unit rebooted under lease",
                        old_boot_id="boot-one", boot_id="boot-two", at=5,
                        holder="agent-a", task="0021"))

    def test_reboot_without_capability_requires_break(self):
        self.acquire()
        original = self.fake.info.read_bytes()
        self.reboot()
        for action in (self.client.check, self.client.renew, self.client.release):
            with self.subTest(action=action.__name__):
                with self.assertRaisesRegex(sl.LeaseError, "unit rebooted under lease"):
                    action("agent-a", "0021")
                self.assertEqual(self.fake.info.read_bytes(), original)
        self.client.break_lease("Owner inspected reboot", "owner", "0021")
        self.assertIsNone(self.client.status())

    def test_reboot_without_matching_identity_or_token_cannot_rebind(self):
        self.acquire(cap="reboot", go="0021")
        original = self.fake.info.read_bytes()
        self.reboot()
        for holder, task, token in (("agent-a", "0021", "b" * 32),
                                    ("agent-b", "0021", None), ("agent-a", "0022", None),
                                    ("agent-a", "0021", "")):
            with self.subTest(holder=holder, task=task, token=token):
                with self.assertRaises(sl.LeaseError):
                    self.client.check(holder, task, token=token)
                self.assertEqual(self.fake.info.read_bytes(), original)
        self.assertFalse((self.fake.data / "lease.log").exists())

    def test_rebind_logging_failure_preserves_record(self):
        self.acquire(cap="install", go="0021")
        original = self.fake.info.read_bytes()
        self.reboot()
        (self.fake.data / "lease.log").mkdir()
        with self.assertRaises(sl.LeaseError):
            self.client.renew("agent-a", "0021")
        self.assertEqual(self.fake.info.read_bytes(), original)

    def test_wall_clock_jumps_do_not_change_heartbeat_age(self):
        original = self.acquire()
        self.now += 120
        for wall in (original.started + 13 * 365 * 86400, 1):
            with self.subTest(wall=wall):
                self.fake.wall_clock = lambda: wall
                self.assertEqual(self.client.check("agent-a", "0021"), original)
                self.assertEqual(json.loads(self.cli("status")[1])["state"], "held")
        renewed = self.client.renew("agent-a", "0021")
        self.assertEqual(renewed, replace(original, heartbeat=1120))
        self.now += sl.EXPIRY + 1
        with self.assertRaisesRegex(sl.LeaseError, "expired"):
            self.client.check("agent-a", "0021")

    def test_missing_info_gives_clear_quarantine_message_before_sed(self):
        self.fake.info.parent.mkdir()
        for action in (self.client.check, self.client.renew, self.client.release):
            with self.subTest(action=action.__name__):
                with self.assertRaisesRegex(sl.LeaseError, "^quarantined lease: incomplete info$"):
                    action("agent-a", "0021")
        with self.assertRaisesRegex(sl.LeaseError, "^quarantined lease: incomplete info$"):
            self.client._raw(live=True)
        self.client.break_lease("Owner inspected incomplete lease", "owner", "0021")
        self.assertIsNone(self.client.status())
