"""Native dependency receipts and the real unit hook share one rebuild decision; no Docker."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import output


class DependencyInputs(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        for name in ('rootfs/deps.sh', 'rootfs/deps.lock', 'tools/output.py', 'boot/unit-hook.sh',
                     'boot/unit-hook-image.sh', 'boot/unit-hook-dtb.sh'):
            dest = self.root / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, dest)
        (self.root / 'rootfs/out').mkdir()
        (self.root / 'kernel').mkdir()
        (self.root / 'kernel/fixture.dtb').touch()
        env = self.root / 'tools/build-env.sh'
        env.write_text('#!/bin/sh\n'
                       'case "$*" in\n'
                       '  "identity rootfs-nft"|"identity rootfs-dropbear"|"identity rootfs-sftp-server")\n'
                       '    echo "fixture-environment:$2" ;;\n'
                       '  *) exit 1 ;;\n'
                       'esac\n')
        env.chmod(0o700)
        self.builders = {'nft': ('nft', 'libmnl libnftnl nftables'),
                         'dropbearmulti': ('dropbear', 'dropbear'), 'sftp-server': ('sftp-server', 'openssh')}
        self.env = {**os.environ, 'CARGO_TARGET_DIR': str(self.root / 'target'),
                    'DEPS_LOCK': str(self.root / 'rootfs/deps.lock'), 'OOS_UNIT': ''}
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        (self.root / '.gitignore').write_text('rootfs/out/\nrebuilds\ntarget/\n')
        for artifact, (builder, names) in self.builders.items():
            script = self.root / f'rootfs/build-{builder}.sh'
            script.write_text('#!/bin/sh\nset -eu\n'
                              'root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)\n'
                              '. "$root/rootfs/deps.sh"\n'
                              f'before=$(python3 "$root/tools/output.py" identity "$root/rootfs/out/{artifact}")\n'
                              f'echo {artifact} >> "$root/rebuilds"\n'
                              f'echo fixture > "$root/rootfs/out/{artifact}"\n'
                              f'dep_stamp "$root/rootfs/out/{artifact}" "$0" {names}\n'
                              f'python3 "$root/tools/output.py" stamp "$root/rootfs/out/{artifact}" "$before"\n')
            script.chmod(0o700)
        self.hook()
        (self.root / 'rebuilds').unlink()

    def hook(self):
        result = subprocess.run(['sh', '-eu', '-c',
                                 'root=$1; here=$root/boot; k=$root/kernel; OOS_KERNEL_DTB=fixture.dtb; '
                                 '. "$root/boot/unit-hook.sh"', 'unit-hook', str(self.root)],
                                env=self.env, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)

    def verify_all(self):
        with patch.object(output, 'ROOT', self.root), patch.dict(os.environ, self.env):
            for name in self.builders:
                artifact = self.root / 'rootfs/out' / name
                record = output.verify(artifact)
                description = artifact.with_name(name + '.stamp').read_bytes()
                self.assertEqual(record['tree'], hashlib.sha256(description).hexdigest())
                self.assertEqual(record['tree'], output.source_identity(artifact))
                builder = self.builders[name][0]
                self.assertRegex(description.decode(), f'(?m)^environment [^\\n]+:rootfs-{builder}$')

    def assert_stale(self, names):
        with patch.object(output, 'ROOT', self.root), patch.dict(os.environ, self.env):
            for name in names:
                with self.assertRaisesRegex(ValueError, 'stale'):
                    output.verify(self.root / 'rootfs/out' / name)

    def test_helper_only_change_rebuilds_all_three_and_renews_receipts(self):
        self.verify_all()
        self.hook()
        self.assertFalse((self.root / 'rebuilds').exists())
        helper = self.root / 'rootfs/deps.sh'
        helper.write_text(helper.read_text() + '\n# Changed shared dependency logic.\n')
        self.assert_stale(self.builders)
        self.hook()
        self.assertEqual(set((self.root / 'rebuilds').read_text().splitlines()), set(self.builders))
        self.verify_all()

    def test_selected_manifest_fields_builder_and_environment_invalidate_both(self):
        for name, before, after in (('rootfs/deps.lock', '2026.94', '2026.95'),
                                    ('rootfs/build-dropbear.sh', 'echo fixture >', 'echo rebuilt >'),
                                    ('tools/build-env.sh', 'fixture-environment', 'new-environment')):
            path = self.root / name
            path.write_text(path.read_text().replace(before, after))
            self.assert_stale(['dropbearmulti'])
            self.hook()
            self.verify_all()
            self.assertIn('dropbearmulti', (self.root / 'rebuilds').read_text().splitlines())
            (self.root / 'rebuilds').unlink()

    def test_unrelated_source_and_unselected_lock_fields_leave_both_current(self):
        (self.root / 'unrelated.rs').write_text('fn unrelated() {}\n')
        lock = self.root / 'rootfs/deps.lock'
        lock.write_text(lock.read_text().replace('https://matt.ucc.asn.au/', 'https://example.invalid/'))
        self.verify_all()
        self.hook()
        self.assertFalse((self.root / 'rebuilds').exists())

    def test_per_tool_environment_change_only_rebuilds_its_tool(self):
        resolver = self.root / 'tools/build-env.sh'
        resolver.write_text(resolver.read_text().replace(
            'echo "fixture-environment:$2"',
            'if [ "$2" = rootfs-dropbear ]; then echo "changed-environment:$2"; '
            'else echo "fixture-environment:$2"; fi'))
        self.assert_stale(['dropbearmulti'])
        with patch.object(output, 'ROOT', self.root), patch.dict(os.environ, self.env):
            for name in ('nft', 'sftp-server'):
                output.verify(self.root / 'rootfs/out' / name)
        self.hook()
        self.assertEqual((self.root / 'rebuilds').read_text().splitlines(), ['dropbearmulti'])
        self.verify_all()

    def test_missing_dependency_or_builder_cannot_produce_a_receipt(self):
        lock = self.root / 'rootfs/deps.lock'
        lock.write_text('\n'.join(line for line in lock.read_text().splitlines() if not line.startswith('dropbear ')))
        with patch.object(output, 'ROOT', self.root), patch.dict(os.environ, self.env):
            with self.assertRaises(subprocess.CalledProcessError):
                output.dependency_inputs('dropbearmulti')
            (self.root / 'rootfs/build-nft.sh').unlink()
            with self.assertRaises(subprocess.CalledProcessError):
                output.dependency_inputs('nft')


if __name__ == '__main__':
    unittest.main()
