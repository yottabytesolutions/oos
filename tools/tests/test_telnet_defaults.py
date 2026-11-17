# SPDX-License-Identifier: GPL-2.0-or-later
"""Exercise the boot telnet switch without starting a listener or booting a unit."""
import os
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class TelnetDefaults(unittest.TestCase):
    def test_boot_requires_explicit_opt_in(self):
        lines = (ROOT / 'rootfs/initramfs/etc/rcS').read_text().splitlines()
        commands = [line for line in lines if line.startswith('case ') and 'remote.telnet' in line]
        self.assertEqual(len(commands), 1)
        command = commands[0].replace('/bin/oos-conf', shlex.quote(str(ROOT / 'rootfs/initramfs/bin/oos-conf')))
        unit = (ROOT / 'rootfs/unit/dho804/system.conf').read_text()
        cases = [
            (None, None, False),
            ('', '', False),
            (unit, None, False),
            ('', 'ntp = on\n', False),
            ('remote.telnet = on\n', 'remote.telnet = off\n', False),
            ('remote.telnet = off\n', 'remote.telnet = on\n', True),
        ]
        cases.extend(('', f'remote.telnet = {value}\n', enabled) for value, enabled in [
            ('on', True), ('true', True), ('1', True),
            ('off', False), ('false', False), ('0', False), ('', False), ('invalid', False),
        ])
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            listener = root / 'telnetd'
            listener.write_text('#!/bin/sh\nprintf "%s\\n" "$*"\n')
            listener.chmod(0o700)
            for image, settings, enabled in cases:
                with self.subTest(image=image, settings=settings):
                    for name, text in [('image', image), ('settings', settings)]:
                        path = root / name
                        if text is None:
                            path.unlink(missing_ok=True)
                        else:
                            path.write_text(text)
                    env = {**os.environ, 'PATH': scratch + os.pathsep + os.environ['PATH'],
                           'OOS_CONF_FILES': f'{root / "image"} {root / "settings"}'}
                    result = subprocess.run(['sh', '-eu', '-c', command], env=env,
                                            check=True, capture_output=True, text=True)
                    self.assertEqual(result.stdout, '-l /bin/sh\n' if enabled else '')


if __name__ == '__main__':
    unittest.main()
