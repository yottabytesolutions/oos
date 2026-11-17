# SPDX-License-Identifier: GPL-2.0-or-later
"""Image applicability from synthetic Git histories; no workflows are changed."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import image_required as image


class Classifier(unittest.TestCase):
    def setUp(self):
        clean_env = {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}
        environment = patch.dict(os.environ, clean_env, clear=True)
        environment.start()
        self.addCleanup(environment.stop)
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git('init', '-q')
        self.git('config', 'user.email', 'fixture@example.invalid')
        self.git('config', 'user.name', 'Fixture')
        self.git('config', 'commit.gpgsign', 'false')
        self.write('README.md'); self.write('boot/old.sh')
        self.git('add', '.'); self.git('commit', '-qm', 'Fixture base')
        self.base = self.git('rev-parse', 'HEAD').decode().strip()

    def git(self, *args):
        return subprocess.check_output(['git', *args], cwd=self.root, stderr=subprocess.PIPE)

    def write(self, name):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('fixture\n')

    def test_docs_only_do_not_require_image(self):
        self.write('docs/explanation.md')
        self.assertFalse(image.required(self.root, self.base)['required'])

    def test_addition_deletion_and_both_rename_paths_require_images(self):
        self.git('mv', 'boot/old.sh', 'moved.md')
        result = image.required(self.root, self.base)
        self.assertTrue(result['required']); self.assertIn('boot/old.sh', result['inputs'])
        self.git('reset', '--hard', '-q', self.base)
        self.git('rm', '-q', 'boot/old.sh')
        self.assertTrue(image.required(self.root, self.base)['required'])
        self.git('reset', '--hard', '-q', self.base)
        self.write('rootfs/nested/new')
        self.assertTrue(image.required(self.root, self.base)['required'])

    def test_lock_config_and_workflow_inputs(self):
        for path in ('Cargo.lock', 'Cargo.toml', 'ui/Cargo.toml', 'new/member/Cargo.toml',
                     '.cargo/config.toml', 'rust-toolchain.toml', 'ui/build.sh',
                     'tools/hw/new.c', 'tools/output.sh', 'tools/build-env/sim-fw.Dockerfile',
                     'tools/container-images.json', '.github/workflows/ci.yml',
                     'tools/image-manifest.py', 'xtask/src/check.rs', 'gpu/compare.py'):
            with self.subTest(path=path):
                self.assertTrue(image.classify(path))
        with self.assertRaisesRegex(ValueError, 'unclassified'):
            image.classify('unclassified/input.config')

    def test_process_files_are_no_image_inputs(self):
        for path in ('.github/dependabot.yml', '.github/labels.yml', '.github/rulesets/main.json',
                     '.github/ISSUE_TEMPLATE/bug.yml', 'tools/ci/codex_gate.py', 'deny.toml', 'cliff.toml'):
            with self.subTest(path=path):
                self.assertFalse(image.classify(path))
        self.assertTrue(image.classify('.github/workflows/pr.yml'))

    def test_unavailable_bases_and_invalid_inventories_fail_closed(self):
        with self.assertRaises(subprocess.CalledProcessError):
            image.required(self.root, 'missing')
        with patch.dict(os.environ, {'PR_BASE': '', 'PUSH_BASE': '0' * 40}):
            with self.assertRaises(ValueError):
                image.required(self.root, ci=True)
        for raw in (b'M\0missing-delimiter', b'R100\0only-one\0', b'bad\0file\0'):
            with self.assertRaises(ValueError):
                image.changed_paths(raw)


if __name__ == '__main__':
    unittest.main()
