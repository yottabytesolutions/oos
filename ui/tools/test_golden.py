# SPDX-License-Identifier: GPL-2.0-or-later
"""Comparator regression tests, invoked by the Cargo golden integration test."""
import os
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch
import struct
import zlib

from golden import (SIGNATURE, compare_rgb, read_png, reference_capture, require_captures, write_diff, write_png,
                    write_report)


class GoldenTests(unittest.TestCase):
    def image(self, n=1000, value=40):
        return n, 1, bytes([value] * n * 3)

    def test_release_check_names_bad_version_hash_and_missing_picture(self):
        from golden_version import check, references
        with tempfile.TemporaryDirectory() as d, patch('golden_version.REFERENCE', Path(d)):
            root = Path(d)
            write_png(root / 'about.png', self.image())
            captures = [
                dict(path='about.png', release_version='0.2.0', rgb_sha256='wrong'),
                dict(path='missing.png', release_version='0.2.1'),
                dict(path='ordinary.png', rgb_sha256='outside the release set'),
            ]
            (root / 'manifest.txt').write_text(json.dumps(dict(captures=captures)))
            _, versions = references()
            self.assertEqual(set(versions), {'about.png', 'missing.png'})
            errors = check(versions, '0.2.1')
            self.assertEqual(len(errors), 3, errors)
            self.assertIn('about.png: manifest.txt records release 0.2.0', errors[0])
            self.assertIn('about.png: decoded RGB differs', errors[1])
            self.assertIn('missing.png:', errors[2])

    def test_release_capture_selection_preserves_frame_steps(self):
        from golden_version import select
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            script = root / 'script.txt'
            script.write_text('frames 2\nshot ordinary.png\ndo About OOS\nshot renamed.png\n')
            row = ('demo/size/menu', 'demo', str(script), 'ordinary.png,renamed.png', 'fixture')
            selected = select(row, {'demo/size/menu/renamed.png': {}}, root / 'stage')
            self.assertEqual(selected[3], 'renamed.png')
            self.assertEqual(Path(selected[2]).read_text(), 'frames 2\nframes 1\ndo About OOS\nshot renamed.png\n')

    def test_reference_demo_labels_map_to_current_metadata(self):
        old = dict(path='demo/default.png', settings='settings dots=Auto Demo acq.pipeline=batch',
                   source_route='Demo records; acq.pipeline=batch', rgb_sha256='unchanged')
        new = dict(old, settings='settings dots=Auto Demo records', source_route='Demo records')
        self.assertEqual(reference_capture(old), new)
        self.assertEqual(reference_capture(new), new)
        batch = dict(old, path='batch/default.png')
        self.assertEqual(reference_capture(batch), batch)

    def test_publish_preserves_foreign_subtrees_and_replaces_owned_captures(self):
        from golden_run import publish
        with tempfile.TemporaryDirectory() as d:
            reference, candidate = Path(d) / 'golden', Path(d) / 'candidate'
            foreign = {'README.md': b'guide', 'update/README.md': b'update guide',
                       'update/pending.png.ref': b'updater', 'demo/foreign/keep.txt': b'foreign'}
            old = {'demo/case/current.png': b'old', 'demo/case/current.meta': b'old metadata',
                   'demo/retired/old.png': b'retired', 'demo/retired/old.meta': b'retired metadata'}
            new = {'demo/case/current.png': b'new', 'demo/case/current.meta': b'new metadata',
                   'demo/new/added.png': b'added', 'demo/new/added.meta': b'added metadata'}
            for tree, files in ((reference, foreign | old), (candidate, new)):
                for name, data in files.items():
                    path = tree / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(data)
                captures = [{'path': name} for name in files if name.endswith('.png')]
                (tree / 'manifest.txt').write_text(json.dumps({'captures': captures}))
            publish(candidate, reference)
            expected = foreign | new | {'manifest.txt': (candidate / 'manifest.txt').read_bytes()}
            actual = {str(p.relative_to(reference)): p.read_bytes() for p in reference.rglob('*') if p.is_file()}
            self.assertEqual(actual, expected)
            self.assertFalse(reference.with_name('golden.pending').exists())
            self.assertFalse(reference.with_name('golden.previous').exists())

    def test_cpu_comparison_requires_every_rgb_byte_to_match(self):
        a = self.image(n=2000)
        regions = [('main', 0, 0, 1000, 1)]
        self.assertTrue(all(m['pass'] for m in compare_rgb(a, a, regions).values()))
        for pixel, region in ((0, 'main'), (1999, 'remaining')):
            for component in range(3):
                for delta in (-1, 1, 2):
                    with self.subTest(pixel=pixel, component=component, delta=delta):
                        b = bytearray(a[2])
                        b[pixel * 3 + component] += delta
                        metrics = compare_rgb(a, (2000, 1, b), regions)
                        self.assertFalse(metrics[region]['pass'])
                        self.assertEqual(metrics[region]['different'], 1)
                        self.assertEqual(metrics[region]['maximum'], abs(delta))
                        other = 'remaining' if region == 'main' else 'main'
                        self.assertTrue(metrics[other]['pass'])

    def test_trace_translation_and_brightness_loss(self):
        a = (100, 100, bytes([0, 200, 0] * 100 + [0, 0, 0] * 9900))
        b = (100, 100, bytes([0, 0, 0] * 100 + [0, 200, 0] * 100 + [0, 0, 0] * 9800))
        self.assertFalse(compare_rgb(a, b)['remaining']['pass'])
        self.assertFalse(compare_rgb(self.image(), self.image(value=20))['remaining']['pass'])
        with self.assertRaises(ValueError):
            compare_rgb(a, self.image())
        with self.assertRaises(ValueError):
            compare_rgb(a, a, [('main', 0, 0, 60, 100), ('zoom', 50, 0, 50, 100)])

    def test_roundtrip_integrity_inventory_and_gallery(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / 'test.png'
            a = self.image()
            write_png(p, a)
            self.assertEqual(read_png(p), a)
            require_captures(d, ['test.png'])
            for names in ([], ['absent.png'], ['test.png', 'extra.png'], ['test.png', 'test.png']):
                with self.assertRaises(ValueError):
                    require_captures(d, names)
            original = p.read_bytes()
            for data in (original[:-1], original + b'x', original[:20] + b'x' + original[21:]):
                p.write_bytes(data)
                with self.assertRaises(ValueError):
                    read_png(p)
            write_report(Path(d) / 'gallery', a, self.image(value=41), compare_rgb(a, self.image(value=41)))
            diff = read_png(Path(d) / 'gallery/diff.png')
            self.assertEqual(diff[2], b'\xff\x00\xff' * 1000)

    def test_child_coverage_is_preserved_without_application_overrides(self):
        from golden_run import child_environment
        with patch.dict(os.environ, {'LLVM_PROFILE_FILE': 'coverage-%p.profraw', 'OOS_INTERP': 'fast'}, clear=True):
            env = child_environment(Path('synthetic'))
            self.assertEqual(env['LLVM_PROFILE_FILE'], 'coverage-%p.profraw')
            self.assertNotIn('OOS_INTERP', env)
        with patch.dict(os.environ, {}, clear=True):
            self.assertNotIn('LLVM_PROFILE_FILE', child_environment(Path('synthetic')))

    def test_prune_retains_only_newest_two_runs(self):
        from golden_run import prune_runs
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            for index in range(4):
                run = root / f'run-{index}'
                run.mkdir()
                os.utime(run, ns=(index + 1, index + 1))
            (root / 'unrelated').mkdir()
            prune_runs(root)
            self.assertEqual({p.name for p in root.iterdir()}, {'run-2', 'run-3', 'unrelated'})

    def test_pruning_cannot_cross_build_profiles(self):
        from golden_run import prune_runs, run_base
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            bases = []
            for coverage in (False, True):
                for profile in ('debug', 'release'):
                    with patch.dict(os.environ, {'LLVM_PROFILE_FILE': 'test-%p.profraw'} if coverage else {}, clear=True):
                        base = run_base(root, profile)
                    bases.append(base)
            self.assertEqual(len(set(bases)), 4)
            for base in bases:
                for index in range(3):
                    run = base / f'run-{index}'
                    run.mkdir(parents=True)
                    os.utime(run, ns=(index + 1, index + 1))
            prune_runs(bases[0], keep=1)
            self.assertEqual(len(list(bases[0].iterdir())), 1)
            for base in bases[1:]:
                self.assertEqual(len(list(base.iterdir())), 3)

    def test_observed_decode_route_and_failure_only_reports(self):
        from golden_run import capture_case
        from types import SimpleNamespace
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            script = root / 'script'
            script.write_text('shot test.png\n')
            image = (1024, 600, bytes(1024 * 600 * 3))
            def fake_child(binary, script, kind, run):
                run.mkdir(parents=True)
                write_png(run / 'test.png', image)
                (run / 'test.png.meta').write_text('main 0 0 1024 600\nsettings menu_open=false\n')
                (run / 'stdout.log').write_text('batch 1 route workers decode decoded\nshot test.png\n')
            row = ['batch/synthetic', 'codes', str(script), 'test.png', 'channels=1;dots=Off;']
            for failing in (False, True):
                metrics = {'main': {'pass': not failing}}
                with patch('golden_run.child', fake_child), patch('golden_run.read_png', return_value=image), \
                     patch('golden_run.compare_rgb', return_value=metrics), patch('golden_run.write_report') as report:
                    captures, failures = capture_case(row, SimpleNamespace(binary='binary', update=False),
                                                      root / str(failing), root / 'candidate')
                    self.assertEqual(report.call_count, int(failing))
                    self.assertEqual(len(failures), int(failing))
                    self.assertEqual(captures[0]['decode_route'], 'decoded')
            self.assertFalse((root / 'candidate').exists())

    def test_all_png_row_filters(self):
        def chunk(kind, data):
            return struct.pack('>I', len(data)) + kind + data + struct.pack('>I', zlib.crc32(kind + data))
        for mode in range(5):
            # Two RGB pixels, two rows; reconstruct [10]*6 for every row.
            rows = {0: bytes([10] * 6), 1: bytes([10] * 3 + [0] * 3),
                    2: bytes([10] * 6), 3: bytes([10] * 3 + [5] * 3), 4: bytes([10] * 3 + [0] * 3)}
            second = {0: bytes([10] * 6), 1: rows[1], 2: bytes(6),
                      3: bytes([5] * 3 + [0] * 3), 4: bytes(6)}
            data = bytes([mode]) + rows[mode] + bytes([mode]) + second[mode]
            with tempfile.TemporaryDirectory() as d:
                p = Path(d) / 'filtered.png'
                p.write_bytes(SIGNATURE + chunk(b'IHDR', struct.pack('>IIBBBBB', 2, 2, 8, 2, 0, 0, 0)) +
                              chunk(b'IDAT', zlib.compress(data)) + chunk(b'IEND', b''))
                self.assertEqual(read_png(p)[2], bytes([10] * 12))


if __name__ == '__main__':
    unittest.main()
