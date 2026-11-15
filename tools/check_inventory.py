"""Default Python and explicit image runners. Status, test counts and artifacts decide success."""
import argparse
import os
from pathlib import Path
import re
import struct
import subprocess
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import output
from script_syntax import tracked

ROOT = output.ROOT
TEST_ROOTS = ('tools/tests', 'tools/privacy', 'ui/tools')


def validate_test_roots(root):
    missing = []
    for name in tracked(root):
        path = Path(name)
        if path.name.startswith('test_') and path.suffix == '.py' and (root / path).is_file():
            if not any(path.is_relative_to(directory) for directory in TEST_ROOTS):
                missing.append(name)
    if missing:
        raise ValueError(f'Python tests outside discovered roots: {missing}; register their runner')


def run(args):
    print('+ ' + ' '.join(map(str, args)), flush=True)
    subprocess.run(list(map(str, args)), cwd=ROOT, check=True)


def elf_static(data):
    if len(data) < 64 or data[:7] != b'\x7fELF\x02\x01\x01':
        raise ValueError('expected little-endian ELF64')
    if struct.unpack_from('<H', data, 16)[0] != 2:
        raise ValueError('expected an ELF executable')
    if struct.unpack_from('<H', data, 18)[0] != 183:
        raise ValueError('expected AArch64 machine')
    offset = struct.unpack_from('<Q', data, 32)[0]
    size, count = struct.unpack_from('<HH', data, 54)
    if not count or size < 56 or offset + size * count > len(data):
        raise ValueError('missing or truncated ELF program headers')
    types = [struct.unpack_from('<I', data, offset + i * size)[0] for i in range(count)]
    if 1 not in types or 3 in types or 2 in types:
        raise ValueError('expected static executable with LOAD and no INTERP/DYNAMIC segments')
    entry = struct.unpack_from('<Q', data, 24)[0]
    executable = False
    for i, kind in enumerate(types):
        if kind != 1:
            continue
        _, flags, start, address, _, length, memory, _ = struct.unpack_from('<IIQQQQQQ', data, offset + i * size)
        if start + length > len(data) or memory < length:
            raise ValueError('truncated ELF LOAD segment')
        executable |= bool(flags & 1 and length and address <= entry < address + length)
    if not executable:
        raise ValueError('ELF entry is outside executable file content')


def arm():
    run(['python3', 'tools/output.py', 'build', '--release', '--target', 'aarch64-unknown-linux-musl',
         '-p', 'oos-ui', '-p', 'oos-fpgad', '--bins'])
    for name in ('oos-ui', 'oos-fpgad', 'oos-update'):
        path = output.target() / 'aarch64-unknown-linux-musl/release' / name
        output.verify(path)
        elf_static(path.read_bytes())
        print(f'arm64-release-link: {name}: receipt and static AArch64 ELF verified')


def nonempty_tests(directory):
    suite = unittest.TestLoader().discover(str(directory), pattern='test_*.py')
    count = suite.countTestCases()
    if not count:
        raise ValueError(f'{directory.name}: zero tests selected')
    return count


def test_summary(text, expected):
    matches = re.findall(r'^Ran (\d+) tests? in ', text, re.MULTILINE)
    if len(matches) != 1 or int(matches[0]) != expected:
        raise ValueError('test selection differs from executed test count')
    skips = re.search(r'\bskipped=(\d+)', text)
    print(f'python-tests: {expected} executed; {skips[1] if skips else 0} skipped')
    reasons = re.findall(r'^.*\.\.\. skipped (.+)$', text, re.MULTILINE)
    for reason in reasons:
        print(f'python-tests: skip reason: {reason}')
    if skips and len(reasons) != int(skips[1]):
        raise ValueError('skipped tests have missing reasons')
    if skips and int(skips[1]) >= expected:
        raise ValueError('all tests were skipped')


def python_tests():
    validate_test_roots(ROOT)
    run(['python3', 'tools/output.py', 'build', '-p', 'oos-ui'])
    output.verify(output.target() / 'debug/oos-ui')
    # Discover in separate interpreters: unittest caches top-level import directories.
    for directory in TEST_ROOTS:
        selection = subprocess.run(['python3', '-I', str(Path(__file__).resolve()), 'count-tests', directory],
                                   cwd=ROOT, check=True, capture_output=True, text=True)
        expected = int(selection.stdout.strip())
        args = ['python3', '-I', '-m', 'unittest', 'discover', '-s', directory, '-p', 'test_*.py', '-v']
        result = subprocess.run(args, cwd=ROOT, capture_output=True, text=True)
        print(result.stdout, end=''); print(result.stderr, end='')
        if result.returncode:
            raise subprocess.CalledProcessError(result.returncode, args)
        test_summary(result.stderr, expected)


def prepare_sim(ssh):
    run(['sh', 'tools/build-env.sh', 'prepare', 'initramfs'])
    run(['sh', 'tools/build-env.sh', 'prepare', 'rootfs'])
    for builder in ('nft', 'dropbear', 'sftp-server') if ssh else ('nft',):
        run(['sh', f'rootfs/build-{builder}.sh'])
    run(['sh', 'tools/ssh-sim.sh' if ssh else 'tools/fw-sim.sh'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('check', choices=('python-tests', 'arm64-release-link', 'ssh-sim', 'firewall-sim', 'count-tests'))
    parser.add_argument('directory', nargs='?')
    args = parser.parse_args()
    os.environ['CARGO_TARGET_DIR'] = str(output.target())
    os.environ.pop('OOS_OUT', None)
    if args.check == 'count-tests':
        print(nonempty_tests(ROOT / args.directory))
    else:
        {'python-tests': python_tests, 'arm64-release-link': arm,
         'ssh-sim': lambda: prepare_sim(True), 'firewall-sim': lambda: prepare_sim(False)}[args.check]()


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as exc:
        sys.exit(f'host check failed: {exc}; see tools/bootstrap.sh and docs/development.md')
