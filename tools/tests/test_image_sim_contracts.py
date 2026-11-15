"""Guest simulation contracts that must fail in the Docker-free default gate."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class SshGuest(unittest.TestCase):
    def test_service_refuses_missing_firewall_before_preparing_ssh(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            service = root / 'oos-ssh'
            source = (ROOT / 'rootfs/initramfs/bin/oos-ssh').read_text()
            service.write_text(source.replace('/run/dropbear.pid', str(root / 'pid')))
            service.chmod(0o700)
            shutil.copy(ROOT / 'rootfs/initramfs/bin/oos-conf', root / 'oos-conf')
            nft = root / 'nft'
            nft.write_text('#!/bin/sh\nprintf "%s\\n" "$*" >> "$OOS_TEST_NFT_CALLS"\nexit 1\n')
            nft.chmod(0o700)
            conf, state, keys, calls = [root / name for name in ('conf', 'state', 'keys', 'calls')]
            env = {**os.environ, 'OOS_CONF_FILES': str(conf), 'OOS_SSH_STATE': str(state),
                   'OOS_SSH_DIR': str(keys), 'OOS_TEST_NFT_CALLS': str(calls)}
            cases = [('# nothing\n', 0, 'off'),
                     ('remote.ssh = on\n', 0, 'off: needs net.allow'),
                     ('remote.ssh = on\nnet.allow = 192.0.2.0/24\n', 1, 'off: firewall not loaded')]
            for config, status, message in cases:
                with self.subTest(message=message):
                    conf.write_text(config)
                    result = subprocess.run(['sh', str(service), 'start'], env=env,
                                            capture_output=True, text=True, timeout=10)
                    self.assertEqual(result.returncode, status, result.stderr)
                    self.assertEqual(state.read_text().strip(), message)
                    self.assertFalse(keys.exists(), 'a refused start must not prepare SSH credentials')
            self.assertEqual(calls.read_text(), 'list table inet oos_fw\n')

    def run_sim_case(self, status, state, listening=False, refusal=True):
        script = (ROOT / 'tools/ssh-sim.sh').read_text()
        guest = script.split("<<'TEST'\n", 1)[1].split('\nTEST\n', 1)[0]
        helpers = guest.split('\nH=', 1)[0]
        case = guest.split('echo "== net.allow, but no firewall"\n', 1)[1]
        case = case.split('echo "== firewall, first start"', 1)[0] if refusal else 'run start\n'
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            service = root / 'oos-ssh'
            service.write_text('#!/bin/sh\nprintf "%s\\n" "$OOS_TEST_STATE" > "$OOS_SSH_STATE"\n'
                               'exit "$OOS_TEST_STATUS"\n')
            service.chmod(0o700)
            commands = helpers + f'\nlistening() {{ return {0 if listening else 1}; }}\n' + case
            for old, name in [('/bin/oos-ssh', 'oos-ssh'), ('/tmp/oos-ssh.state', 'state'),
                              ('/tmp/oos-ssh.log', 'log'), ('/etc/oos/system.conf', 'conf')]:
                commands = commands.replace(old, str(root / name))
            env = {**os.environ, 'OOS_TEST_STATUS': str(status), 'OOS_TEST_STATE': state,
                   'OOS_SSH_STATE': str(root / 'state')}
            return subprocess.run(['sh', '-u', '-c', commands + '\n[ "$ok" = 1 ]\n'],
                                  env=env, capture_output=True, text=True, timeout=10)

    def test_sim_requires_exact_refusal_state_and_no_listener(self):
        for status, state, listening, expected in [
                (1, 'off: firewall not loaded', False, 0),
                (0, 'off: firewall not loaded', False, 1),
                (2, 'off: firewall not loaded', False, 1),
                (1, 'off', False, 1),
                (1, 'off: firewall not loaded', True, 1)]:
            with self.subTest(status=status, state=state, listening=listening):
                result = self.run_sim_case(status, state, listening)
                self.assertEqual(result.returncode, expected, result.stdout + result.stderr)

    def test_sim_still_rejects_unexpected_start_failures(self):
        for status in (0, 1, 2):
            with self.subTest(status=status):
                result = self.run_sim_case(status, 'unused', refusal=False)
                self.assertEqual(result.returncode, int(status != 0), result.stdout + result.stderr)


class WlanGuest(unittest.TestCase):
    def test_live_radio_invocation_sets_guest_opt_in(self):
        script = (ROOT / 'tools/wlan-sim.sh').read_text()
        guest = script.split("<<'INIT'\n", 1)[1].split('\nINIT\n', 1)[0]
        commands = [line for line in guest.splitlines() if 'ui::wlan::tests::live_radio' in line]
        self.assertEqual(len(commands), 1, 'exactly one guest WLAN test must execute')
        with tempfile.TemporaryDirectory() as scratch:
            timeout = Path(scratch) / 'timeout'
            timeout.write_text('#!/usr/bin/env python3\nimport json, os, sys\n'
                               'print(json.dumps([os.environ.get("OOS_WLAN_SIM"), sys.argv[1:]]))\n')
            timeout.chmod(0o700)
            for inherited in (None, '0'):
                with self.subTest(inherited=inherited):
                    env = {**os.environ, 'PATH': scratch + os.pathsep + os.environ['PATH']}
                    env.pop('OOS_WLAN_SIM', None)
                    if inherited is not None:
                        env['OOS_WLAN_SIM'] = inherited
                    result = subprocess.run(['sh', '-eu', '-c', commands[0]], env=env,
                                            check=True, capture_output=True, text=True)
                    opt_in, args = json.loads(result.stdout)
                    self.assertEqual(opt_in, '1', 'WLAN opt-in must reach the guest test process')
                    self.assertEqual(args, ['200', '/bin/oos-ui-test', '--ignored', '--exact',
                                           '--nocapture', '--test-threads=1', 'ui::wlan::tests::live_radio'])


if __name__ == '__main__':
    unittest.main()
