# SPDX-License-Identifier: GPL-2.0-or-later
"""GPU comparison regressions use synthetic pictures and script shot lists."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class GPURegression(unittest.TestCase):
    def compare(self, a, b):
        return subprocess.run([sys.executable, '-I', str(ROOT / 'gpu/compare.py'), str(a), str(b)],
                              capture_output=True, text=True)

    def test_empty_comparison_fails(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertNotEqual(self.compare(Path(d), Path(d)).returncode, 0)

    def test_equal_sets_must_still_match_every_script_shot(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            a, b, scripts = root / 'cpu', root / 'gpu', root / 'scripts'
            for directory in (a, b, scripts):
                directory.mkdir()
            for directory in (a, b):
                (directory / 'one.ppm').write_bytes(b'P6\n1 1\n255\n\x00\x00\x00')
            script = scripts / 'fixture.txt'
            script.write_text('shot $S/one.ppm\n')
            args = [sys.executable, '-I', str(ROOT / 'gpu/compare.py'), str(a), str(b), str(scripts)]
            self.assertEqual(subprocess.run(args, capture_output=True).returncode, 0)
            script.write_text('shot $S/one.ppm\nshot $S/missing.ppm\n')
            self.assertNotEqual(subprocess.run(args, capture_output=True).returncode, 0)

    def test_extra_and_truncated_pictures_fail(self):
        with tempfile.TemporaryDirectory() as d:
            a, b = Path(d) / 'cpu', Path(d) / 'gpu'
            a.mkdir(); b.mkdir()
            ppm = b'P6\n1 1\n255\n\x00\x00\x00'
            (a / 'one.ppm').write_bytes(ppm)
            (b / 'one.ppm').write_bytes(ppm)
            self.assertEqual(self.compare(a, b).returncode, 0)
            (b / 'extra.ppm').write_bytes(ppm)
            self.assertNotEqual(self.compare(a, b).returncode, 0)
            (b / 'extra.ppm').unlink()
            (a / 'one.ppm').write_bytes(ppm[:-1])
            self.assertNotEqual(self.compare(a, b).returncode, 0)


if __name__ == '__main__':
    unittest.main()
