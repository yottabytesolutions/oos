"""Regression evidence from real subprocesses, with no container or hardware access."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]


class Processes(unittest.TestCase):
    def test_gel_dry_run_assertion_fails_instead_of_only_printing(self):
        script = (ROOT / 'tools/gel-sim.sh').read_text()
        lines = script.splitlines()
        assertion = next(i for i, line in enumerate(lines) if '[ "$h0" =' in line)
        code = lines[assertion].replace('$(sha256sum < /w/card)', 'changed')
        result = subprocess.run(['sh', '-c', 'h0=original\n' + code + '\necho PASS'], capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn(b'PASS', result.stdout)

    def test_cargo_check_is_not_a_release_link(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / 'src').mkdir(); (root / '.cargo').mkdir()
            (root / 'Cargo.toml').write_text('[package]\nname="link-fixture"\nversion="0.0.0"\nedition="2021"\n')
            (root / 'src/main.rs').write_text('extern "C" { fn oos_missing_fixture_symbol(); }\nfn main() { unsafe { oos_missing_fixture_symbol(); } }\n')
            shutil.copyfile(ROOT / '.cargo/config.toml', root / '.cargo/config.toml')
            pin = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel']
            env = {**os.environ, 'CARGO_TARGET_DIR': str(root / 'build')}
            args = ['cargo', '+' + pin]
            def run(*command):
                return subprocess.run([*args, *command], cwd=root, env=env, capture_output=True, text=True)
            self.assertEqual(run('generate-lockfile', '--offline').returncode, 0)
            checked = run('check', '--offline', '--locked', '--target', 'aarch64-unknown-linux-musl')
            self.assertEqual(checked.returncode, 0, checked.stderr)
            linked = run('build', '--offline', '--locked', '--release', '--target', 'aarch64-unknown-linux-musl')
            self.assertNotEqual(linked.returncode, 0)
            self.assertIn('oos_missing_fixture_symbol', linked.stderr)

    def test_qemu_status_and_wlan_test_status_cannot_be_hidden_by_tee(self):
        for name in ('usb', 'wlan'):
            script = (ROOT / f'tools/{name}-sim.sh').read_text()
            qemu = script[script.index('qemu-system-aarch64 -M'):]
            self.assertNotIn('| tee', qemu)
            self.assertIn('|| { cat "$w/log"; exit 1; }', qemu)
        script = (ROOT / 'tools/wlan-sim.sh').read_text()
        self.assertIn("grep -q '^WLAN TEST EXIT 0'", script)
        self.assertIn('> "$w/cargo.json"', script)
        self.assertNotIn('| python3 -c', script)


if __name__ == '__main__':
    unittest.main()
