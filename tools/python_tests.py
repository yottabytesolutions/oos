# SPDX-License-Identifier: GPL-2.0-or-later
"""Isolated unittest discovery, execution counts and skip evidence for CI and xtask."""
from pathlib import Path
import re
import subprocess
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import script_syntax

ROOTS = ('tools/tests', 'tools/privacy', 'ui/tools')
COUNT = "import sys, unittest; print(unittest.TestLoader().discover(sys.argv[1], pattern='test_*.py').countTestCases())"
# Fixture skips have IDs such as "setUpClass (test_release_artifacts.ReleaseArtifacts)".
RELEASE_ARTIFACT_ID = re.compile(r'^(?:\w+ \()?test_release_artifacts(?:[.)]|$)')


def validate_roots(root):
    missing = [name for name in script_syntax.tracked(root)
               if Path(name).match('test_*.py') and (root / name).is_file()
               and not any(Path(name).is_relative_to(directory) for directory in ROOTS)]
    if missing:
        raise ValueError(f'Python tests outside discovered roots: {missing}; register their runner')


def summary(text, expected):
    counts = [int(n) for n in re.findall(r'^Ran ([0-9]+) tests? in ', text, re.MULTILINE)]
    if expected == 0 or counts != [expected]:
        raise ValueError('test selection differs from executed test count')
    skip_count = re.search(r'skipped=([0-9]+)', text)
    skipped = int(skip_count[1]) if skip_count else 0
    reasons = [line.split('... skipped ', 1)[1] for line in text.splitlines()
               if '... skipped ' in line and line.split('... skipped ', 1)[1]]
    if skip_count and len(reasons) != skipped:
        raise ValueError('skipped tests have missing reasons')
    if skipped >= expected:
        raise ValueError('all tests were skipped')
    return [f'python-tests: {expected} executed; {skipped} skipped',
            *(f'python-tests: skip reason: {reason}' for reason in reasons)]


def _run_discovery(directory):
    suite = unittest.TestLoader().discover(directory, pattern='test_*.py')
    result = unittest.TextTestRunner(verbosity=2, warnings='default').run(suite)
    if directory == 'tools/tests':
        release_skips = sum(bool(RELEASE_ARTIFACT_ID.match(test.id())) for test, _ in result.skipped)
        print(f'python-tests: test_release_artifacts: {release_skips} skipped', flush=True)
        if release_skips:
            sys.exit('python-tests: test_release_artifacts must run without skips; '
                     'set OOS_RELEASE_TEST_UPDATER to the built host oos-update')
    return int(not result.wasSuccessful())


def run_root(root, directory):
    selection = subprocess.run([sys.executable, '-I', '-c', COUNT, directory], cwd=root,
                               capture_output=True, text=True, check=True)
    expected = int(selection.stdout.strip())
    if expected == 0:
        raise ValueError(f'{directory}: zero tests selected')
    result = subprocess.run([sys.executable, '-I', str(Path(__file__).resolve()), '--discover', directory],
                            cwd=root, capture_output=True, text=True)
    print(result.stdout + result.stderr, end='', flush=True)
    result.check_returncode()
    for line in summary(result.stderr, expected):
        print(line, flush=True)


def run(root):
    validate_roots(root)
    for directory in ROOTS:
        run_root(root, directory)


if __name__ == '__main__':
    try:
        if sys.argv[1:2] == ['--discover']:
            sys.exit(_run_discovery(sys.argv[2]))
        run(Path(__file__).resolve().parents[1])
    except (ValueError, OSError, subprocess.SubprocessError) as exc:
        sys.exit(f'python-tests: {exc}')
