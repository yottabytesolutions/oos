# SPDX-License-Identifier: GPL-2.0-or-later
"""Script discovery and syntax checks never execute script bodies."""
import os
from pathlib import Path
import py_compile
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import script_syntax


class Discovery(unittest.TestCase):
    def setUp(self):
        clean_env = {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}
        environment = patch.dict(os.environ, clean_env, clear=True)
        environment.start()
        self.addCleanup(environment.stop)
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        self.write('valid.py', 'x = 1\n')
        self.write('valid.sh', '#!/bin/sh\nx=1\n')

    def write(self, name, text, mode=0o600):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text); path.chmod(mode)
        return path

    def test_nested_invalid_python_and_rootfs_shell_fail(self):
        path = self.write('tools/nested/a space.py', 'if broken\n')
        with self.assertRaises(py_compile.PyCompileError):
            script_syntax.check(self.root, self.root / 'out')
        path.unlink()
        self.write('rootfs/initramfs/bin/extensionless', '#!/bin/sh\nif then\n', 0o700)
        with self.assertRaises(subprocess.CalledProcessError):
            script_syntax.check(self.root, self.root / 'out')

    def test_syntax_does_not_execute_and_keeps_bytecode_under_target(self):
        self.write('a space/danger.py', 'raise RuntimeError("never execute")\n')
        self.write('rootfs/entry', '#!/usr/bin/env bash\n[[ x == x ]]\n', 0o700)
        self.write('.githooks/hook', '#!/bin/sh\nexit 7\n', 0o700)
        script_syntax.check(self.root, self.root / 'out')
        self.assertTrue((self.root / 'out/script-bytecode/a space/danger.pyc').exists())
        self.assertFalse((self.root / 'a space/__pycache__').exists())

    def test_unknown_executable_and_unmapped_fragment_fail(self):
        for name, text in [('tools/new', 'echo unchecked\n'), ('tools/new.sh', 'x=1\n'),
                           ('tools/new', '#!/usr/bin/env perl\n1;\n'),
                           ('tools/new', '#!/usr/bin/env python3-unclassified\nx=1\n'),
                           ('tools/new', '#!/system/bin/shutdown\nx=1\n'),
                           ('tools/new', '#!/bin/busybox shutdown\nx=1\n')]:
            path = self.write(name, text, 0o700)
            with self.assertRaises(ValueError):
                script_syntax.interpreter(path, name)
            path.unlink()
        for name, text, shell in [('tools/output.sh', 'x=1\n', 'sh'),
                                  ('boot/gel/do_extract.sh', '#!/system/bin/sh\n', 'mksh'),
                                  ('tools/new', '#!/usr/bin/env -S bash -e\n', 'bash')]:
            path = self.write(name, text)
            self.assertEqual(script_syntax.interpreter(path, name), shell)

    def test_empty_git_script_selection_fails(self):
        (self.root / 'valid.py').unlink()
        (self.root / 'valid.sh').unlink()
        with self.assertRaisesRegex(ValueError, 'empty Git discovery'):
            script_syntax.check(self.root, self.root / 'out')


if __name__ == '__main__':
    unittest.main()
