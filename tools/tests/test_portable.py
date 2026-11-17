"""tools/lib/portable.sh with each hash tool alone on PATH, as on Linux, the BSDs and macOS."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
HELPER = ROOT / 'tools/lib/portable.sh'
BASICS = ('awk', 'wc', 'cat')
TOOLS = {'sha256sum': 'sha256sum', 'shasum': 'shasum', 'openssl': 'openssl'}


def fake_path(tmp, tool):
    """A PATH with the basics and one hash tool, so the helper must take that branch."""
    bin_dir = Path(tmp) / f'bin-{tool}'
    bin_dir.mkdir()
    for name in dict.fromkeys(BASICS + (tool,)):
        found = shutil.which(name)
        if found is None:
            return None
        (bin_dir / name).symlink_to(found)
    return str(bin_dir)


def run(script, path, cwd, stdin=None, args=()):
    return subprocess.run(['/bin/sh', '-c', f'. "{HELPER}"; {script}', 'sh', *args], cwd=cwd, env={'PATH': path},
                          input=stdin, capture_output=True, text=True, check=False)


class Portable(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.data = Path(self.tmp.name) / 'a file'
        self.data.write_bytes(bytes(range(256)) * 300)
        self.want = hashlib.sha256(self.data.read_bytes()).hexdigest()

    def each_tool(self):
        tried = 0
        for tool in TOOLS:
            path = fake_path(self.tmp.name, tool)
            if path is None:
                continue
            tried += 1
            with self.subTest(tool=tool):
                yield path
        if tried == 0:
            self.skipTest('no SHA-256 tool on this host')

    def test_sha256_of_a_file_and_of_stdin(self):
        for path in self.each_tool():
            got = run('sha256 "$1"; sha256 < "$1"', path, self.tmp.name, args=(str(self.data),))
            self.assertEqual(got.returncode, 0, got.stderr)
            self.assertEqual(got.stdout.split(), [self.want, self.want])

    def test_sha256_of_a_missing_file_fails(self):
        for path in self.each_tool():
            got = run('sha256 missing', path, self.tmp.name)
            self.assertNotEqual(got.returncode, 0)
            self.assertEqual(got.stdout, '')

    def test_sha256_check(self):
        for path in self.each_tool():
            good = run('sha256_check', path, self.tmp.name, f'{self.want}  {self.data.name}\n')
            self.assertEqual(good.returncode, 0, good.stderr)
            binary = run('sha256_check', path, self.tmp.name, f'{self.want} *{self.data.name}\n')
            self.assertEqual(binary.returncode, 0, binary.stderr)
            bad = run('sha256_check', path, self.tmp.name, f'{"0" * 64}  {self.data.name}\n')
            self.assertNotEqual(bad.returncode, 0)
            self.assertIn('expected', bad.stderr)
            empty = run('sha256_check', path, self.tmp.name, '')
            self.assertNotEqual(empty.returncode, 0)

    def test_file_bytes(self):
        path = fake_path(self.tmp.name, 'wc') or os.defpath
        got = run('file_bytes "$1"', path, self.tmp.name, args=(str(self.data),))
        self.assertEqual(got.returncode, 0, got.stderr)
        self.assertEqual(got.stdout, f'{256 * 300}\n')
        missing = run('file_bytes missing', path, self.tmp.name)
        self.assertNotEqual(missing.returncode, 0)


if __name__ == '__main__':
    unittest.main()
