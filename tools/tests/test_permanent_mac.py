# SPDX-License-Identifier: GPL-2.0-or-later
"""The resolved device MAC reaches GMAC before network registration."""
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
MAC = '00:00:5e:00:53:04'


class PermanentMAC(unittest.TestCase):
    def test_probe_parameter_preserves_the_dtb_mac_hash_input(self):
        patch = (ROOT / 'kernel/patches/0013-net-rockchip-permanent-mac-at-probe.patch').read_text()
        added = '\n'.join(line[1:] for line in patch.splitlines()
                          if line.startswith('+') and not line.startswith('+++'))
        helper = re.search(r'static int rk_gmac_set_mac\([^}]+\}', added).group()
        # Compile the actual patch helper against host Ethernet primitives. The output is the six bytes
        # passed to stmmac, the same registration input previously supplied by local-mac-address.
        harness = r'''
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include <errno.h>
#define ETH_ALEN 6
struct stmmac_resources { unsigned char mac[ETH_ALEN]; };
static char *macaddr;
static bool mac_pton(const char *s, unsigned char *out) {
    unsigned int b[6];
    if (sscanf(s, "%2x:%2x:%2x:%2x:%2x:%2x", &b[0], &b[1], &b[2], &b[3], &b[4], &b[5]) != 6)
        return false;
    for (int i = 0; i < 6; i++) out[i] = b[i];
    return true;
}
static bool is_valid_ether_addr(const unsigned char *a) {
    const unsigned char zero[6] = {0};
    return !(a[0] & 1) && memcmp(a, zero, 6);
}
HELPER
int main(int argc, char **argv) {
    struct stmmac_resources res = {{0}};
    macaddr = argc > 1 ? argv[1] : NULL;
    if (argc > 2) mac_pton(argv[2], res.mac);
    if (rk_gmac_set_mac(&res)) return 1;
    for (int i = 0; i < 6; i++) printf("%02x", res.mac[i]);
    return 0;
}
'''.replace('HELPER', helper)
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp) / 'probe.c'
            source.write_text(harness)
            program = Path(tmp) / 'probe'
            subprocess.run(['cc', '-Wall', '-Wextra', '-Werror', str(source), '-o', str(program)], check=True)
            for previous in [[], [MAC], ['00:00:5e:00:53:05']]:
                result = subprocess.run([str(program), MAC, *previous], capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(bytes.fromhex(result.stdout), bytes.fromhex(MAC.replace(':', '')))
            multicast = f'{int(MAC[:2], 16) | 1:02x}{MAC[2:]}'
            for invalid in [[], [''], ['invalid'], ['00:00:00:00:00:00'],
                            [multicast], [MAC + ':00']]:
                result = subprocess.run([str(program), *invalid], capture_output=True)
                self.assertNotEqual(result.returncode, 0, invalid)
        self.assertLess(patch.index('+\tret = rk_gmac_set_mac'), patch.index(' \t/* If the stmmac'))
        self.assertNotIn('ipv6', (ROOT / 'kernel/series').read_text())
        self.assertFalse((ROOT / 'kernel/patches/0013-ipv6-stable-address-uses-selected-mac.patch').exists())

    def test_driver_is_deferred_and_packaged(self):
        config = (ROOT / 'kernel/config').read_text()
        self.assertIn('CONFIG_DWMAC_ROCKCHIP=m\n', config)
        board = (ROOT / 'rootfs/board/dho/etc/oos-board').read_text()
        modules = re.search(r'^OOS_MODULE_NAMES="([^"]+)"', board, re.M).group(1).split()
        self.assertIn('dwmac-rockchip', modules)
        for path in (ROOT / 'kernel/dts').glob('*.dts'):
            self.assertNotIn('local-mac-address', path.read_text())

    def test_board_passes_mac_at_probe_and_checks_the_registered_address(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            board = (ROOT / 'rootfs/board/dho/bin/oos-board').read_text()
            board = board.replace('/sys/class/net/eth0/address', str(root / 'address'))
            (root / 'board').write_text(board)
            (root / 'modprobe').write_text('#!/bin/sh\nprintf "%s\\n" "$*" > "$(dirname "$0")/args"\n')
            (root / 'modprobe').chmod(0o755)
            env = {**os.environ, 'PATH': str(root) + os.pathsep + os.environ['PATH']}
            for address, code in [(MAC, 0), ('00:00:5e:00:53:05', 1), ('', 1)]:
                (root / 'address').write_text(address + '\n')
                result = subprocess.run(['sh', str(root / 'board'), 'ethernet', MAC.upper()], env=env,
                                        capture_output=True, text=True)
                self.assertEqual(result.returncode, code, result.stderr)
                self.assertEqual((root / 'args').read_text(), f'dwmac-rockchip macaddr={MAC.upper()}\n')
            (root / 'modprobe').write_text('#!/bin/sh\nexit 1\n')
            (root / 'address').write_text(MAC + '\n')
            result = subprocess.run(['sh', str(root / 'board'), 'ethernet', MAC], env=env)
            self.assertNotEqual(result.returncode, 0)


if __name__ == '__main__':
    unittest.main()
