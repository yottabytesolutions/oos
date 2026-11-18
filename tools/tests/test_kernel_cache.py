# SPDX-License-Identifier: GPL-2.0-or-later
"""Kernel cache keys, publication and reuse with synthetic build outputs."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import kernel_cache as cache

ROOT = Path(__file__).resolve().parents[2]
IMAGE = 'sha256:' + '1' * 64
PLATFORM = 'linux/arm64'
ENVIRONMENT = 'environment'


class KernelCache(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        self.store = self.root / 'cache'
        self.store.mkdir()
        for source in cache.source_files():
            destination = self.root / source.relative_to(ROOT)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)
        self.source_patch = patch.object(cache, 'ROOT', self.root)
        self.source_patch.start()
        self.addCleanup(self.source_patch.stop)
        resolver = patch.object(cache.build_env, 'resolve', return_value=IMAGE)
        resolver.start()
        self.addCleanup(resolver.stop)

    def key(self, image=IMAGE, platform=PLATFORM, environment=ENVIRONMENT):
        return cache.input_key(image, platform, environment)

    def outputs(self, directory):
        directory.mkdir(parents=True, exist_ok=True)
        for name in cache.required_outputs():
            (directory / name).write_text(ENVIRONMENT if name == 'build-environment' else name)

    def builder(self, args, **kwargs):
        self.assertEqual(args[:3], ['sh', str(self.root / 'kernel/build-base.sh'), '_build'])
        self.assertTrue(kwargs['check'])
        self.outputs(Path(args[3]))

    def build(self):
        cache.build(self.store, IMAGE, PLATFORM, ENVIRONMENT)

    def test_every_input_and_container_identity_invalidate_but_outputs_do_not(self):
        before = self.key()
        for source in cache.source_files():
            with self.subTest(input=source.relative_to(self.root)):
                old = source.read_bytes()
                try:
                    source.write_bytes(old.replace(old[:64], b'0' * 64, 1)
                                       if source.name == 'base.sha256' else old + b'\nchanged\n')
                    self.assertNotEqual(before, self.key())
                finally:
                    source.write_bytes(old)
        self.assertNotEqual(before, self.key(image='sha256:' + '2' * 64))
        self.assertNotEqual(before, self.key(platform='linux/amd64'))
        self.assertNotEqual(before, self.key(environment='other'))
        self.outputs(self.root / 'kernel/out/base')
        (self.root / 'ui.rs').write_text('unrelated Rust edit')
        self.assertEqual(before, self.key())

    def test_additions_deletions_and_renames_invalidate(self):
        before = self.key()
        for name in ('patches/new.patch', 'dts/new.dts', 'config.d/extra', 'modules/new.c'):
            with self.subTest(input=name):
                source = self.root / 'kernel' / name
                source.parent.mkdir(parents=True, exist_ok=True)
                source.write_text('new input')
                self.assertNotEqual(before, self.key())
                source.unlink()
                self.assertEqual(before, self.key())
        config = self.root / 'kernel/config'
        config.rename(config.with_name('config-renamed'))
        self.assertNotEqual(before, self.key())

    def test_executable_mode_and_present_archive_are_checked(self):
        before = self.key()
        answers = self.root / 'kernel/tools/git'
        answers.chmod(answers.stat().st_mode ^ 0o111)
        self.assertNotEqual(before, self.key())
        archive = self.root / 'kernel/src/linux-6.12.115-base.tar.xz'
        archive.parent.mkdir()
        archive.write_bytes(b'wrong source archive')
        with self.assertRaisesRegex(ValueError, 'checksum mismatch'):
            self.key()
        (self.root / 'kernel/base.sha256').write_text(cache.sha256(archive) + '  src/' + archive.name + '\n')
        self.assertIsInstance(self.key(), str)

    def test_keys_are_shared_across_worktrees(self):
        other = self.root / 'other-worktree'
        other.mkdir()
        for source in cache.source_files():
            destination = other / source.relative_to(self.root)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)
        before = self.key()
        with patch.object(cache, 'ROOT', other):
            self.assertEqual(before, self.key())

    def test_location_uses_absolute_xdg_then_platform_default(self):
        with patch.dict(os.environ, {'XDG_CACHE_HOME': str(self.store)}):
            self.assertEqual(cache.cache_dir(), self.store / 'oos/kernel-base')
        with patch.dict(os.environ, {'XDG_CACHE_HOME': 'relative'}), patch.object(Path, 'home', return_value=self.root):
            with patch.object(cache.sys, 'platform', 'darwin'):
                self.assertEqual(cache.cache_dir(), self.root / 'Library/Caches/oos/kernel-base')
            with patch.object(cache.sys, 'platform', 'linux'):
                self.assertEqual(cache.cache_dir(), self.root / '.cache/oos/kernel-base')

    def test_hit_restores_all_outputs_without_building_or_source_archive(self):
        with patch.object(cache.subprocess, 'run', side_effect=self.builder) as run:
            self.build()
            shutil.rmtree(self.root / 'kernel/out')
            self.build()
            self.assertEqual(run.call_count, 1)
        self.assertTrue(cache.valid(self.root / 'kernel/out/base', self.key(), ENVIRONMENT))

    def test_miss_replaces_outputs_and_reports_size(self):
        with patch.object(cache.subprocess, 'run', side_effect=self.builder) as run:
            self.build()
            old_key = self.key()
            (self.root / 'kernel/config').write_text('changed config')
            with patch('builtins.print') as report:
                self.build()
            self.assertEqual(run.call_count, 2)
        self.assertTrue(cache.valid(self.store / old_key, old_key, ENVIRONMENT))
        self.assertTrue(cache.valid(self.root / 'kernel/out/base', self.key(), ENVIRONMENT))
        self.assertIn('bytes', report.call_args.args[0])

    def test_corrupt_wrong_key_missing_and_unrecorded_outputs_are_rebuilt(self):
        with patch.object(cache.subprocess, 'run', side_effect=self.builder) as run:
            self.build()
            entry = self.store / self.key()
            for corruption in ('payload', 'key', 'missing', 'receipt', 'extra', 'malformed'):
                with self.subTest(corruption=corruption):
                    if corruption == 'payload':
                        (entry / 'Image').write_text('corrupt')
                    elif corruption == 'key':
                        receipt = json.loads((entry / cache.RECEIPT).read_text())
                        receipt['key'] = 'wrong'
                        (entry / cache.RECEIPT).write_text(json.dumps(receipt))
                    elif corruption == 'missing':
                        (entry / 'modules.tar.gz').unlink()
                    elif corruption == 'receipt':
                        (entry / cache.RECEIPT).write_text('{}')
                    elif corruption == 'extra':
                        (entry / 'old.dtb').write_text('unrecorded')
                    else:
                        (entry / cache.RECEIPT).write_text('{"key": "wrong", "outputs": []}')
                    self.build()
                    self.assertTrue(cache.valid(entry, self.key(), ENVIRONMENT))
            self.assertEqual(run.call_count, 7)

    def test_verify_rejects_stale_or_replaced_local_outputs(self):
        with patch.object(cache.subprocess, 'run', side_effect=self.builder):
            self.build()
        with patch.object(cache.sys, 'argv', ['kernel_cache.py', 'verify']), \
                patch.object(cache.build_env, 'platform_for', return_value=PLATFORM), \
                patch.object(cache.build_env, 'identity', return_value=({}, 'tag', ENVIRONMENT)):
            cache.main()
            image = self.root / 'kernel/out/base/Image'
            image.write_text('replaced')
            with self.assertRaisesRegex(ValueError, 'stale or incomplete'):
                cache.main()
            image.write_text('Image')
            (self.root / 'kernel/config').write_text('changed source')
            with self.assertRaisesRegex(ValueError, 'stale or incomplete'):
                cache.main()

    def test_failed_build_does_not_publish_or_replace_previous_outputs(self):
        with patch.object(cache.subprocess, 'run', side_effect=self.builder):
            self.build()
        previous = cache.output_hashes(self.root / 'kernel/out/base')
        (self.root / 'kernel/config').write_text('new config')
        with patch.object(cache.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, 'builder')):
            with self.assertRaises(subprocess.CalledProcessError):
                self.build()
        self.assertFalse((self.store / self.key()).exists())
        self.assertEqual(previous, cache.output_hashes(self.root / 'kernel/out/base'))
        self.assertFalse(list(self.store.glob('.build-*')))

    def test_missing_required_output_and_midbuild_input_change_reject_publication(self):
        def changed(args, **kwargs):
            self.builder(args, **kwargs)
            (self.root / 'kernel/config').write_text('midbuild change')
        def incomplete(args, **kwargs):
            Path(args[3]).mkdir()
        for builder in (incomplete, changed):
            with self.subTest(builder=builder), patch.object(cache.subprocess, 'run', side_effect=builder):
                with self.assertRaises(ValueError):
                    self.build()
            self.assertEqual(list(self.store.iterdir()), [])

    def test_midbuild_container_change_rejects_publication(self):
        with patch.object(cache.subprocess, 'run', side_effect=self.builder), \
                patch.object(cache.build_env, 'resolve', return_value='sha256:' + '2' * 64):
            with self.assertRaisesRegex(ValueError, 'inputs changed'):
                self.build()
        self.assertFalse((self.store / self.key()).exists())

    def test_concurrent_builds_publish_once_and_purge_preserves_lock(self):
        program = '''import pathlib, sys
sys.path.insert(0, sys.argv[1])
import kernel_cache as c
c.ROOT = pathlib.Path(sys.argv[2])
c.cache_dir = lambda: c.ROOT / 'cache'
c.build_env.platform_for = lambda role: 'linux/arm64'
c.build_env.identity = lambda *args: (None, None, 'environment')
c.build_env.prepare = lambda *args: 'sha256:' + '1' * 64
def builder(args, **kwargs):
    print('building', flush=True)
    sys.stdin.readline()
    out = pathlib.Path(args[3])
    out.mkdir()
    for name in c.required_outputs():
        (out / name).write_text('environment' if name == 'build-environment' else name)
c.subprocess.run = builder
c.build_env.resolve = lambda *args: 'sha256:' + '1' * 64
c.build_env.platform_for = lambda *args: 'linux/arm64'
c.build_env.identity = lambda *args: ({}, 'fixture', 'environment')
sys.argv = ['kernel_cache.py', sys.argv[3]]
c.main()
'''
        command = [sys.executable, '-c', program, str(ROOT / 'tools'), str(self.root)]
        first = subprocess.Popen([*command, 'build'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.addCleanup(self.stop_process, first)
        self.assertIn('cache miss', first.stdout.readline())
        self.assertEqual(first.stdout.readline().strip(), 'building')
        second = subprocess.Popen([*command, 'build'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.addCleanup(self.stop_process, second)
        first_output, _ = first.communicate('\n', timeout=30)
        second_output, _ = second.communicate('\n', timeout=30)
        self.assertEqual((first.returncode, second.returncode), (0, 0))
        self.assertIn('cache stored', first_output)
        self.assertIn('cache hit', second_output)
        result = subprocess.run([*command, 'purge'], capture_output=True, text=True, check=True)
        self.assertIn('purged', result.stdout)
        self.assertEqual([p.name for p in self.store.iterdir()], ['.lock'])

    @staticmethod
    def stop_process(process):
        if process.poll() is None:
            process.kill()
        process.communicate()


if __name__ == '__main__':
    unittest.main()
