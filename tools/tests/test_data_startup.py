# SPDX-License-Identifier: GPL-2.0-or-later
"""Exercise the startup notice and mount decision using regular files only."""
from pathlib import Path
import os
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
REPAIRED = "The data area was repaired"


class DataStartup(unittest.TestCase):
    def setUp(self):
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.root = Path(scratch.name)
        self.notice = self.root / "notice"
        self.kmsg = self.root / "kmsg"
        self.mount = self.root / "mount-attempted"
        self.kmsg.touch()
        (self.root / "area").touch()
        (self.root / "mounts").touch()
        (self.root / "start").write_text("516096\n")
        (self.root / "size").write_text("32768\n")
        self.board = self.root / "board"
        self.board.write_text("OOS_OOSDATA_LBA=516096\nOOS_OOSDATA_SECTORS=32768\n")

        source = (ROOT / "rootfs/initramfs/bin/oos-mount-data").read_text()
        paths = {
            "/sys/class/block/mmcblk0p1": str(self.root),
            "/dev/mmcblk0p1": str(self.root / "area"),
            "/dev/mmcblk0": str(self.root / "card"),
            "/proc/mounts": str(self.root / "mounts"),
            "/tmp/oos-data.notice": str(self.notice),
            "/dev/kmsg": str(self.kmsg),
            "[ -b $P ]": "[ -f $P ]",
            "/oos": str(self.root / "oos"),
        }
        source = re.sub("|".join(map(re.escape, paths)), lambda m: paths[m.group()], source)
        self.script = self.root / "startup"
        self.script.write_text(source)

        self.stub("oos-update", """case "$2" in
    clean-recovered) exit 0 ;;
esac
[ -z "$REPAIR_NOTICE" ] || printf '%s\n' "$REPAIR_NOTICE"
exit "$REPAIR_EXIT"
""")
        self.stub("mount", 'printf "mounted\n" > "$MOUNT_ATTEMPT"\nexit "$MOUNT_EXIT"\n')

    def stub(self, name, body):
        path = self.root / name
        path.write_text("#!/bin/sh\n" + body)
        path.chmod(0o700)

    def run_startup(self, repair_notice="", repair_exit=0, mount_exit=0, stock_confirmed=True):
        env = {
            **os.environ,
            "PATH": f"{self.root}{os.pathsep}{os.environ['PATH']}",
            "OOS_BOARD_FILE": str(self.board),
            "REPAIR_NOTICE": repair_notice,
            "REPAIR_EXIT": str(repair_exit),
            "MOUNT_EXIT": str(mount_exit),
            "MOUNT_ATTEMPT": str(self.mount),
        }
        return subprocess.run(["sh", str(self.script), str(int(stock_confirmed))], env=env,
                              capture_output=True, text=True)

    def test_repaired_data_mounts_and_reports_to_panel_and_kernel(self):
        result = self.run_startup(REPAIRED)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.mount.exists())
        self.assertEqual(self.notice.read_text().strip(), REPAIRED)
        self.assertIn(REPAIRED, self.kmsg.read_text())

    def test_clean_data_mounts_without_notice(self):
        result = self.run_startup()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(self.mount.exists())
        self.assertFalse(self.notice.exists())

    def test_failed_repair_does_not_mount_and_explains_unavailable_data(self):
        result = self.run_startup(repair_exit=1)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertFalse(self.mount.exists())
        message = self.notice.read_text().strip()
        self.assertIn("Saved settings and the calibration copy are unavailable", message)
        self.assertIn("Network identity is temporary for this start", message)
        self.assertIn("Power-cycle to start stock Android", message)
        self.assertIn("contact support", message)
        self.assertIn(message, self.kmsg.read_text())

    def test_unconfirmed_selection_does_not_promise_a_stock_start(self):
        result = self.run_startup(repair_exit=1, stock_confirmed=False)
        self.assertEqual(result.returncode, 1, result.stderr)
        message = self.notice.read_text().strip()
        self.assertIn("Next start unknown", message)
        self.assertIn("run oos-oneshot successfully, then power-cycle", message)

    def test_failed_mount_reports_unavailable_data_even_after_repair(self):
        result = self.run_startup(REPAIRED, mount_exit=1)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertTrue(self.mount.exists())
        self.assertIn("The data area is unavailable", self.notice.read_text())
