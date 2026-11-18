# SPDX-License-Identifier: GPL-2.0-or-later
"""Release source names follow build metadata without contacting a release service."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / 'tools/release-sources.sh'


class ReleaseSources(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.target = Path(tmp.name)
        self.version = self.target / 'image/busybox.version'
        self.version.parent.mkdir()

    def run_script(self, *args):
        return subprocess.run(['sh', str(SCRIPT), *map(str, args)],
                              env={**os.environ, 'CARGO_TARGET_DIR': str(self.target)},
                              capture_output=True, text=True)

    def test_source_names_follow_each_builds_glibc_version(self):
        for version in ('2.44-0ubuntu1', '2.44-0ubuntu1.1'):
            with self.subTest(version=version):
                self.version.write_text(f'busybox-static 1.37.0-1ubuntu1\nBuilt-Using: glibc (= {version})\n')
                result = self.run_script('list')
                self.assertEqual(result.returncode, 0, result.stderr)
                exact = [line for line in result.stdout.splitlines() if line.startswith('glibc_2.')]
                self.assertEqual(exact, [f'glibc_{version}.dsc', 'glibc_2.44.orig.tar.*',
                                         f'glibc_{version}.debian.tar.*'])

    def test_explicit_metadata_path_overrides_output_directory(self):
        version = self.target / 'other build.version'
        version.write_text('Built-Using: glibc (= 2.45-0ubuntu2)\n')
        result = self.run_script('list', version)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('glibc_2.45-0ubuntu2.dsc', result.stdout.splitlines())

    def test_missing_metadata_fails_before_listing_or_contacting_a_release(self):
        for args in (('list',), ('check', 'v0.0.0')):
            with self.subTest(args=args):
                result = self.run_script(*args)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, '')

    def test_metadata_without_build_glibc_is_rejected(self):
        self.version.write_text('busybox-static 1.37.0-1ubuntu1\n')
        result = self.run_script('list')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('missing build glibc', result.stderr)
        self.assertEqual(result.stdout, '')


if __name__ == '__main__':
    unittest.main()
