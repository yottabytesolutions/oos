# SPDX-License-Identifier: GPL-2.0-or-later
"""Unit build identity staging, without Docker or hardware access."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[2]


class UnitIdentity(unittest.TestCase):
    def test_unit_and_plain_builds_replace_the_staged_identity(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            kernel = root / "kernel"
            kernel.mkdir()
            (kernel / "plain.dtb").touch()
            (kernel / "unit.dtb").touch()
            unit = root / "rootfs/unit/test-unit"
            unit.mkdir(parents=True)
            (unit / "build.env").write_text("OOS_KERNEL_DTB=unit.dtb\n")
            (unit / "limits.conf").write_text("afg = no\n")
            (root / "rootfs/deps.sh").write_text("dep_stale() { return 1; }\n")
            mac = root / "rootfs/initramfs/bin/oos-mac"
            mac.parent.mkdir(parents=True)
            mac.write_text("exit 0\n")
            hooks = root / "boot"
            hooks.mkdir()
            for name in ("unit-hook.sh", "unit-hook-image.sh", "unit-hook-dtb.sh"):
                shutil.copyfile(REPO / "boot" / name, hooks / name)
            script = 'set -eu; root=$1; here=$root/boot; k=$root/kernel; OOS_KERNEL_DTB=plain.dtb; . "$here/unit-hook.sh"'
            for name in ("test-unit", "", "test-unit"):
                result = subprocess.run(["sh", "-c", script, "test", str(root)],
                                        env={**os.environ, "OOS_UNIT": name}, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                stage = root / "rootfs/out/unit-hook"
                self.assertEqual((stage / "unit").read_text(), (name or "none") + "\n")
                self.assertEqual((stage / "etc-oos/limits.conf").exists(), bool(name))
            for name in ("../escape", "two words", "x" * 65):
                result = subprocess.run(["sh", "-c", script, "test", str(root)],
                                        env={**os.environ, "OOS_UNIT": name}, capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
