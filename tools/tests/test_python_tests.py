# SPDX-License-Identifier: GPL-2.0-or-later
"""Python gate selection and execution evidence, without a Rust toolchain."""
import contextlib
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import python_tests

PASS = "import sys, unittest\nassert sys.flags.isolated\nclass Tests(unittest.TestCase):\n    def test_pass(self): pass\n"


class PythonTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        return path

    def test_summary_requires_exact_nonzero_counts_and_every_skip_reason(self):
        self.assertEqual(python_tests.summary('Ran 1 test in 0.1s\nOK', 1),
                         ['python-tests: 1 executed; 0 skipped'])
        skipped = "test_fixture (fixture.Tests) ... skipped 'requires fixture'\nRan 2 tests in 0.1s\nOK (skipped=1)"
        self.assertEqual(python_tests.summary(skipped, 2),
                         ['python-tests: 2 executed; 1 skipped', "python-tests: skip reason: 'requires fixture'"])
        for text, expected in [('Ran 0 tests in 0s', 0), ('Ran 0 tests in 0s', 2), ('PASS', 2),
                               ('Ran 1 test in 0s', 2), ('Ran 2 tests in 0s\nRan 2 tests in 0s', 2),
                               ('Ran 2 tests in 0s\nOK (skipped=1)', 2),
                               ("a ... skipped 'why'\nb ... skipped 'why'\nRan 2 tests in 0s\nOK (skipped=2)", 2)]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                python_tests.summary(text, expected)

    def test_unregistered_tests_fail_and_deleted_files_are_ignored(self):
        paths = [f'{directory}/test_fixture.py' for directory in python_tests.ROOTS]
        for name in paths:
            self.write(name, PASS)
        with patch.object(python_tests.script_syntax, 'tracked', return_value=paths):
            python_tests.validate_roots(self.root)
            for name in ('test_new.py', 'nested/test_new.py', 'tools/test_new.py', 'tools/tests-other/test_new.py'):
                with self.subTest(name=name):
                    path = self.write(name, 'pass\n')
                    paths.append(name)
                    with self.assertRaisesRegex(ValueError, name):
                        python_tests.validate_roots(self.root)
                    path.unlink()
                    python_tests.validate_roots(self.root)

    def test_isolated_roots_with_matching_module_names(self):
        paths = [f'{directory}/test_fixture.py' for directory in python_tests.ROOTS]
        for name in paths:
            self.write(name, PASS)
        output = io.StringIO()
        with patch.object(python_tests.script_syntax, 'tracked', return_value=paths), contextlib.redirect_stdout(output):
            python_tests.run(self.root)
        self.assertEqual(output.getvalue().count('python-tests: 1 executed; 0 skipped'), len(python_tests.ROOTS))

    def test_empty_failed_discovery_failed_tests_and_all_skipped_runs_fail(self):
        directory = 'tests'
        self.write(f'{directory}/empty.py', '')
        with self.assertRaisesRegex(ValueError, 'zero tests'):
            python_tests.run_root(self.root, directory)
        for text in ("raise RuntimeError('broken import')\n",
                     "import unittest\nclass Tests(unittest.TestCase):\n    def test_fail(self): self.fail('FAIL')\n",
                     "import unittest\n"
                     "@unittest.skip('fixture unavailable')\nclass Tests(unittest.TestCase):\n    def test_skip(self): pass\n"):
            self.write(f'{directory}/test_fixture.py', text)
            with self.subTest(text=text), contextlib.redirect_stdout(io.StringIO()):
                with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                    python_tests.run_root(self.root, directory)

    def test_only_release_artifact_skips_fail_the_gate(self):
        directory = 'tools/tests'
        skip = ("import unittest\nclass Tests(unittest.TestCase):\n"
                "    @unittest.skip('fixture unavailable')\n"
                "    def test_skip(self):\n        \"Test with a description.\"\n")
        self.write(f'{directory}/test_other.py', skip)
        self.write(f'{directory}/test_release_artifacts.py', PASS)
        with contextlib.redirect_stdout(io.StringIO()) as output:
            python_tests.run_root(self.root, directory)
        self.assertIn('test_release_artifacts: 0 skipped', output.getvalue())
        self.assertIn('2 executed; 1 skipped', output.getvalue())

        self.write(f'{directory}/test_release_artifacts.py',
                   PASS + skip.replace('class Tests', 'class SkippedTests') +
                   "class SkippedClass(unittest.TestCase):\n"
                   "    @classmethod\n    def setUpClass(cls):\n        raise unittest.SkipTest('fixture unavailable')\n"
                   "    def test_skip(self): pass\n")
        with contextlib.redirect_stdout(io.StringIO()) as output:
            with self.assertRaises(subprocess.CalledProcessError):
                python_tests.run_root(self.root, directory)
        self.assertIn('test_release_artifacts: 2 skipped', output.getvalue())
        self.assertIn('OOS_RELEASE_TEST_UPDATER', output.getvalue())

    def test_failed_count_and_selection_execution_mismatch_fail(self):
        selection = subprocess.CompletedProcess([], 0, stdout='2\n', stderr='')
        result = subprocess.CompletedProcess([], 0, stdout='', stderr='Ran 1 test in 0s\nOK\n')
        with patch.object(python_tests.subprocess, 'run', side_effect=[selection, result]):
            with contextlib.redirect_stdout(io.StringIO()), self.assertRaisesRegex(ValueError, 'test count'):
                python_tests.run_root(self.root, 'tests')
        with patch.object(python_tests.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, 'python3')):
            with self.assertRaises(subprocess.CalledProcessError):
                python_tests.run_root(self.root, 'tests')


if __name__ == '__main__':
    unittest.main()
