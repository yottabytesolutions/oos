# SPDX-License-Identifier: GPL-2.0-or-later
"""Telnet has no boot path or generated default, even for legacy settings."""
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


class TelnetRemoved(unittest.TestCase):
    def test_boot_and_shipped_defaults_have_no_telnet_service(self):
        for name in ('rootfs/initramfs/etc/rcS', 'rootfs/initramfs/etc/oos/system.conf',
                     'rootfs/initramfs/bin/oos-firewall'):
            text = (ROOT / name).read_text()
            self.assertNotIn('telnet', text, name)
        self.assertNotRegex((ROOT / 'rootfs/initramfs/bin/oos-firewall').read_text(),
                            r'dport\s+23\b')


if __name__ == '__main__':
    unittest.main()
