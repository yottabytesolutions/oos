# SPDX-License-Identifier: GPL-2.0-or-later
"""Exercise deployment and restoration through a simulated SSH transport."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class DeviceDeploy(unittest.TestCase):
    def setUp(self):
        scratch = self.enterContext(tempfile.TemporaryDirectory())
        self.root = Path(scratch)
        self.calls = self.root / 'calls'
        self.repo = self.root / 'repo'
        (self.repo / 'tools/lib').mkdir(parents=True)
        for name in ('device-deploy.sh', 'device-deploy-ssh.sh', 'lib/portable.sh',
                     'plan-matrix.sh', 'plan-matrix-cases.txt'):
            (self.repo / 'tools' / name).write_bytes((ROOT / 'tools' / name).read_bytes())
        (self.repo / 'tools/device-deploy.sh').chmod(0o700)
        (self.repo / 'tools/output.sh').write_text('')
        (self.repo / 'tools/output.py').write_text('')
        self.script('repo/tools/scope-lease', 'exit 0')
        self.env = {**os.environ, 'PATH': scratch + os.pathsep + os.environ['PATH'],
                    'CALLS': str(self.calls), 'CARGO_TARGET_DIR': str(self.root / 'target'),
                    'OOS_SSH_IDENTITY': 'test identity', 'OOS_SSH_PORT': '2222'}

    def script(self, name, body):
        path = self.root / name
        path.write_text('#!/bin/sh\n' + body + '\n')
        path.chmod(0o700)
        return str(path)

    def deploy(self, entry, *args):
        return subprocess.run(['sh', str(self.repo / 'tools' / entry), *args],
                              env=self.env, capture_output=True, text=True, timeout=10)

    def test_restore_uses_ssh_without_a_transport_fallback(self):
        self.script('ssh', 'printf "%s\\n" "$*" >> "$CALLS"; exit "$SSH_STATUS"')
        self.script('nc', 'echo nc >> "$CALLS"; exit 1')
        for entry, args in [('device-deploy.sh', ['--host', 'example.invalid']),
                            ('device-deploy-ssh.sh', ['example.invalid'])]:
            for status in ('0', '255'):
                with self.subTest(entry=entry, status=status):
                    self.calls.write_text('')
                    self.env['SSH_STATUS'] = status
                    result = self.deploy(entry, *args, '--restore')
                    self.assertEqual(result.returncode, int(status), result.stderr)
                    commands = self.calls.read_text().splitlines()
                    self.assertEqual(len(commands), 3 if status == '0' else 1)
                    for command in commands:
                        self.assertIn('-o BatchMode=yes -o StrictHostKeyChecking=yes', command)
                        self.assertIn('-p 2222 -i test identity root@example.invalid', command)

    def test_deploy_stops_legacy_telnet_before_starting_the_new_ui(self):
        self.env.update(OOS_DEPLOY_UI=self.script('oos-ui', 'exit 0'),
                        OOS_DEPLOY_FPGAD=self.script('oos-fpgad', 'exit 0'),
                        UPLOAD=str(self.root / 'upload'),
                        PORTABLE=str(self.repo / 'tools/lib/portable.sh'))
        self.script('sleep', 'exit 0')
        self.script('ssh', r'''
for command do :; done
case "$command" in
    'umask '*) cat > "$UPLOAD" ;;
    'sha256sum '*)
        . "$PORTABLE"
        printf '%s  %s\n' "$(sha256 "$UPLOAD")" "${command#sha256sum }" ;;
    *telnetd*)
        pidof() {
            printf 'lookup %s\n' "$1" >> "$CALLS"
            [ "$1" = telnetd ] && [ -n "$TELNET_PIDS" ] && printf '%s\n' "$TELNET_PIDS"
        }
        kill() { printf 'signal %s\n' "$@" >> "$CALLS"; }
        eval "$command" ;;
    *'/tmp/oos-ui.new --hw'*) echo 'start new UI' >> "$CALLS" ;;
esac''')
        for pids in ('', '123 456'):
            with self.subTest(telnet_pids=pids):
                self.calls.write_text('')
                self.env['TELNET_PIDS'] = pids
                result = self.deploy('device-deploy.sh', '--host', 'example.invalid')
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(self.calls.read_text().splitlines(),
                                 ['lookup telnetd', *(f'signal {pid}' for pid in pids.split()), 'start new UI'])


if __name__ == '__main__':
    unittest.main()
