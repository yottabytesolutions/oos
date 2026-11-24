# SPDX-License-Identifier: GPL-2.0-or-later
"""Generic images obtain identity on the device and follow LAN address changes."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
BIN = ROOT / 'rootfs/initramfs/bin'
MAC = '00:00:5e:00:53:04'


class DeviceIdentity(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        for name in ('oos-mac', 'oos-conf', 'oos-net6', 'oos-firewall'):
            shutil.copyfile(BIN / name, self.root / name)
            (self.root / name).chmod(0o755)
        board = self.root / 'oos-board'
        board.write_text('#!/bin/sh\nprintf "%s\\n" "$BOARD_MAC"\n')
        board.chmod(0o755)
        sync = self.root / 'sync'
        sync.write_text('#!/bin/sh\nexit 0\n')
        sync.chmod(0o755)
        self.conf = self.root / 'system.conf'
        self.cpu = self.root / 'cpuinfo'
        self.cpu.write_text('Serial\t: 0123456789abcdef\n')
        self.env = {**os.environ, 'OOS_DEVICE_CONF': str(self.conf), 'OOS_CPUINFO': str(self.cpu),
                    'OOS_CPU_NVMEM': str(self.root / 'nvmem'), 'BOARD_MAC': '',
                    'PATH': str(self.root) + os.pathsep + os.environ['PATH']}

    def run_script(self, name, *args):
        return subprocess.run(['sh', str(self.root / name), *args], env=self.env,
                              capture_output=True, text=True, timeout=10)

    def mac(self):
        result = self.run_script('oos-mac')
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout.strip()

    def test_saved_mac_wins_and_migration_keeps_device_state_byte_for_byte(self):
        before = f'# Existing device overrides\neth.mac = {MAC}\nntp = off\n'.encode()
        self.conf.write_bytes(before)
        secret = self.root / 'net/ipv6.secret'
        secret.parent.mkdir()
        original = b'0123:4567:89ab:cdef:0123:4567:89ab:cdef\n'
        secret.write_bytes(original)
        # A legacy installed image name has no role in MAC selection.
        (self.root / 'oos-unit').write_text('old-unit\n')
        self.env['BOARD_MAC'] = '00:00:5e:00:53:05'
        self.assertEqual(self.mac(), MAC)
        self.assertEqual(self.conf.read_bytes(), before)
        result = self.run_script('oos-net6', '--secret', str(secret))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('secret kept', result.stderr)
        self.assertEqual(secret.read_bytes(), original)

    def test_first_boot_hardware_mac_is_persisted_once(self):
        self.env['BOARD_MAC'] = MAC
        self.assertEqual(self.mac(), MAC)
        before = self.conf.read_bytes()
        self.assertTrue(before.startswith(b'# OOS device identity'))
        self.assertEqual(before.count(b'eth.mac ='), 1)
        self.env['BOARD_MAC'] = '00:00:5e:00:53:05'
        self.assertEqual(self.mac(), MAC)
        self.assertEqual(self.conf.read_bytes(), before)

    def test_mac_replacement_keeps_existing_settings_if_staging_sync_fails(self):
        self.env['BOARD_MAC'] = MAC
        old = b'# Existing device overrides\nntp = off\n'
        self.conf.write_bytes(old)
        (self.root / 'sync').write_text('#!/bin/sh\ncase "$1" in *.new) exit 1 ;; esac\n')
        self.assertNotEqual(self.run_script('oos-mac').returncode, 0)
        self.assertEqual(self.conf.read_bytes(), old)

    def test_cpu_mac_is_stable_unicast_local_and_independent_of_card(self):
        first = self.mac()
        self.assertEqual(int(first[:2], 16) & 3, 2)
        self.conf.unlink()
        self.env['OOS_CID_FILE'] = '/nonexistent'
        self.assertEqual(self.mac(), first)
        self.conf.unlink()
        self.cpu.write_text('Serial\t: 0123456789abcdee\n')
        self.assertNotEqual(self.mac(), first)

    def test_nvmem_fallback_and_missing_identity_fails_without_randomness(self):
        self.cpu.write_text('Serial : 0000000000000000\n')
        result = self.run_script('oos-mac')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.conf.exists())
        (self.root / 'nvmem').write_bytes(bytes(7) + bytes(range(16)))
        first = self.mac()
        self.conf.unlink()
        self.assertEqual(self.mac(), first)

    def test_existing_keys_survive_first_identity_write_and_bad_mac_is_not_replaced(self):
        self.conf.write_text('ntp = off\n# keep\n')
        self.mac()
        self.assertTrue(self.conf.read_text().startswith('ntp = off\n# keep\n'))
        self.conf.write_text('eth.mac = invalid\n')
        self.assertNotEqual(self.run_script('oos-mac').returncode, 0)
        self.assertEqual(self.conf.read_text(), 'eth.mac = invalid\n')

    def test_boot_never_brings_ethernet_up_with_a_random_fallback(self):
        self.cpu.write_text('')
        rc = (ROOT / 'rootfs/initramfs/etc/rcS').read_text()
        start = rc.index('if mac=$(/bin/oos-mac')
        end = rc.index('# 3) remote shells', start)
        script = rc[start:end].replace('/bin/oos-mac', str(self.root / 'oos-mac'))
        script = script.replace('/tmp/oos-net.log', str(self.root / 'net.log'))
        script = script.replace('/bin/oos-board', str(self.root / 'oos-board'))
        (self.root / 'oos-board').write_text('#!/bin/sh\nexit 1\n')
        ip = self.root / 'ip'
        ip.write_text('#!/bin/sh\necho "$*" >> "$OOS_DEVICE_CONF.ip"\nexit 1\n')
        ip.chmod(0o755)
        for hardware in ['', MAC]:
            self.env['BOARD_MAC'] = hardware
            result = subprocess.run(['sh', '-c', script], env=self.env, capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            calls = Path(str(self.conf) + '.ip')
            if calls.exists():
                self.assertNotIn('eth0 up', calls.read_text())
            self.assertIn('Ethernet left down', (self.root / 'net.log').read_text())

    def test_resolved_mac_is_passed_to_probe_before_ipv6_and_link_up(self):
        rc = (ROOT / 'rootfs/initramfs/etc/rcS').read_text()
        start = rc.index('if mac=$(/bin/oos-mac')
        # Exercise the foreground activation; DHCP below it runs separately after the link is up.
        script = rc[start:rc.index('\n\t(', start)] + '\nfi\n'
        for name in ('oos-mac', 'oos-conf', 'oos-board', 'oos-net6'):
            script = script.replace('/bin/' + name, str(self.root / name))
        script = script.replace('/tmp/oos-net.log', str(self.root / 'net.log'))
        for name, event in [('oos-net6', 'ipv6'), ('ip', 'link $*')]:
            path = self.root / name
            path.write_text(f'#!/bin/sh\necho "{event}" >> "$OOS_DEVICE_CONF.events"\n')
            path.chmod(0o755)
        board = self.root / 'oos-board'
        board.write_text('#!/bin/sh\ncase "$1" in\nmac) echo "$BOARD_MAC";;\n'
                         'ethernet) echo "probe $2" >> "$OOS_DEVICE_CONF.events";;\nesac\n')
        self.env['OOS_CONF_FILES'] = str(self.conf)
        for saved, hardware in [(MAC, '00:00:5e:00:53:05'), ('', MAC), ('', '')]:
            with self.subTest(saved=saved, hardware=hardware):
                self.conf.write_text('net.ipv6 = on\n' + (f'eth.mac = {saved}\n' if saved else ''))
                self.env['BOARD_MAC'] = hardware
                expected = self.mac()
                events = Path(str(self.conf) + '.events')
                events.write_text('')
                result = subprocess.run(['sh', '-c', script], env=self.env,
                                        capture_output=True, text=True, timeout=10)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(events.read_text().splitlines(),
                                 [f'probe {expected}', 'ipv6', 'link link set eth0 up'])

    def test_explicit_legacy_selectors_keep_the_existing_mac(self):
        cid = self.root / 'cid'
        cid.write_text('00112233445566778899aabbccddeeff\n')
        self.env['OOS_CID_FILE'] = str(cid)
        self.conf.write_text('eth.mac = cid\n')
        first = self.mac()
        self.env['BOARD_MAC'] = MAC
        self.assertEqual(self.mac(), first)
        self.assertEqual(self.conf.read_text(), 'eth.mac = cid\n')
        self.conf.write_text('eth.mac = vendor\n')
        self.assertEqual(self.mac(), MAC)
        self.assertEqual(self.conf.read_text(), 'eth.mac = vendor\n')

    def test_watcher_replaces_and_removes_ipv4_subnets_on_each_tick(self):
        self.conf.write_text('net.allow = onlink\n')
        routes = self.root / 'routes4'
        routes.write_text('192.0.2.0/24 dev eth0 proto kernel scope link src 192.0.2.5\n')
        prefixes = self.root / 'oos-net4.prefixes'
        prefixes.write_text('192.0.2.0/24\n')
        watcher = self.root / 'oos-firewall'
        watcher.write_text(watcher.read_text().replace('/bin/nft', str(self.root / 'nft'))
                           .replace('/tmp/oos-', str(self.root / 'oos-')))
        nft = self.root / 'nft'
        nft.write_text('#!/bin/sh\ncat "$2" >> "$TEST_ROOT/applied.nft"\n')
        nft.chmod(0o755)
        # Each fake sleep is a completion event, independent of elapsed time.
        tick = self.root / 'sleep'
        tick.write_text('''#!/bin/sh
n=$(cat "$TEST_ROOT/count" 2>/dev/null || echo 0)
n=$((n + 1)); echo "$n" > "$TEST_ROOT/count"
case "$n" in
1) echo '198.51.100.0/24 dev eth0 proto kernel scope link src 198.51.100.5' > "$OOS_ROUTES4";;
2) cp "$TEST_ROOT/oos-net4.prefixes" "$TEST_ROOT/first"; : > "$OOS_ROUTES4";;
3) cp "$TEST_ROOT/oos-net4.prefixes" "$TEST_ROOT/second"; kill -TERM "$PPID";;
esac
''')
        tick.chmod(0o755)
        self.env.update(OOS_CONF_FILES=str(self.conf), OOS_ROUTES4=str(routes), TEST_ROOT=str(self.root))
        result = self.run_script('oos-firewall', 'watch')
        self.assertEqual(result.returncode, -15, result.stderr)
        self.assertEqual((self.root / 'first').read_text(), '198.51.100.0/24\n')
        self.assertFalse((self.root / 'second').read_text().strip())
        applied = (self.root / 'applied.nft').read_text()
        self.assertEqual(applied.count('flush set inet oos_fw onlink\n'), 2)
        self.assertIn('add element inet oos_fw onlink { 198.51.100.0/24 }', applied)
        self.assertNotIn('192.0.2.', applied)

    def test_hislip_firewall_follows_its_switch(self):
        routes = self.root / 'routes'
        routes.write_text('')
        self.env.update(OOS_CONF_FILES=str(self.conf), OOS_ROUTES4=str(routes), OOS_ROUTES6=str(routes))
        for value in [None, 'off', 'on', 'off', 'true', '1', '', 'invalid']:
            self.conf.write_text('net.allow = 192.0.2.0/24 2001:db8::/64\n' +
                                 ('' if value is None else f'remote.hislip = {value}\n'))
            result = self.run_script('oos-firewall', '--print')
            self.assertEqual(result.returncode, 0, result.stderr)
            blocked = 'tcp dport 4880 drop'
            self.assertEqual(blocked in result.stdout, value not in ('on', 'true', '1'))
            self.assertIn('ip saddr { 192.0.2.0/24 } accept', result.stdout)
            self.assertIn('ip6 saddr { 2001:db8::/64 } accept', result.stdout)
            if blocked in result.stdout:
                self.assertLess(result.stdout.index(blocked), result.stdout.index('ct state established'))
                self.assertLess(result.stdout.index(blocked), result.stdout.index('ip saddr'))

    def test_generic_defaults_and_firewall_follow_connected_routes(self):
        defaults = (ROOT / 'rootfs/initramfs/etc/oos/system.conf').read_text()
        self.assertIn('net.allow = onlink 169.254.0.0/16 fe80::/10 onlink6\n', defaults)
        self.assertNotIn('eth.mac', defaults)
        self.assertNotIn('192.0.2.', defaults)
        self.conf.write_text(defaults)
        routes = self.root / 'routes4'
        routes6 = self.root / 'routes6'
        routes6.write_text('')
        self.env.update(OOS_CONF_FILES=str(self.conf), OOS_ROUTES4=str(routes), OOS_ROUTES6=str(routes6))
        for network, previous in [('192.0.2.0', '198.51.100.0'), ('198.51.100.0', '192.0.2.0')]:
            routes.write_text(f'{network}/24 dev eth0 proto kernel scope link src {network[:-1]}5\n'
                              'default via 203.0.113.1 dev eth0\n'
                              '203.0.113.0/24 via 203.0.113.1 dev eth0\n')
            result = self.run_script('oos-firewall', '--print')
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn(f'elements = {{ {network}/24 }}', result.stdout)
            self.assertIn('ip saddr @onlink accept', result.stdout)
            self.assertNotIn(previous, result.stdout)
            self.assertNotIn('203.0.113.', result.stdout)


if __name__ == '__main__':
    unittest.main()
