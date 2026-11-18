# SPDX-License-Identifier: GPL-2.0-or-later
"""Content-addressed kernel outputs; only complete, verified entries may be reused."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parent))
import build_env

ROOT = Path(__file__).resolve().parents[1]
RECEIPT = 'build-cache.json'


def cache_dir():
    xdg = os.environ.get('XDG_CACHE_HOME', '')
    base = Path(xdg) if Path(xdg).is_absolute() else (
        Path.home() / 'Library/Caches' if sys.platform == 'darwin' else Path.home() / '.cache')
    return base / 'oos/kernel-base'


def sha256(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def source_files():
    kernel = ROOT / 'kernel'
    files = [p for p in kernel.rglob('*') if p.is_file()
             and p.relative_to(kernel).parts[0] not in ('src', 'out')
             and p.name != 'README.md']
    files += [ROOT / name for name in (
        'tools/kernel_cache.py', 'tools/build-env.sh', 'tools/build_env.py',
        'tools/build-env/kernel.Dockerfile', 'tools/container-images.json', 'tools/lib/portable.sh')]
    return sorted(files)


def input_key(image, platform, environment):
    # The archive is identified by its checked manifest, so hits need no download.
    expected, name = (ROOT / 'kernel/base.sha256').read_text().split()
    archive = ROOT / 'kernel' / name
    if archive.exists() and sha256(archive) != expected:
        raise ValueError('kernel source checksum mismatch')
    inputs = {str(p.relative_to(ROOT)): [sha256(p), p.stat().st_mode & 0o111]
              for p in source_files()}
    inputs.update(image=image, platform=platform, environment=environment)
    return hashlib.sha256(json.dumps(inputs, sort_keys=True).encode()).hexdigest()


def required_outputs():
    return {'Image', 'config', 'kernel.release', 'modules.tar.gz', 'SHA256SUMS',
            'build-environment', 'build-packages.txt',
            *(p.stem + '.dtb' for p in (ROOT / 'kernel/dts').glob('*.dts'))}


def output_hashes(directory):
    return {p.name: sha256(p) for p in sorted(directory.iterdir())
            if p.is_file() and p.name != RECEIPT}


def valid(directory, key, environment):
    try:
        receipt = json.loads((directory / RECEIPT).read_text())
        if not isinstance(receipt, dict) or not isinstance(receipt.get('outputs'), dict):
            return False
        return (receipt['key'] == key and required_outputs() <= receipt['outputs'].keys()
                and (directory / 'build-environment').read_text().strip() == environment
                and receipt['outputs'] == output_hashes(directory))
    except (OSError, ValueError, KeyError, TypeError):
        return False


def restore(entry):
    out = ROOT / 'kernel/out/base'
    out.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.restore-', dir=out.parent) as scratch:
        pending = Path(scratch) / 'base'
        shutil.copytree(entry, pending)
        if out.exists():
            out.rename(Path(scratch) / 'previous')
        pending.rename(out)


def report(entry, state):
    size = sum(p.stat().st_size for p in entry.iterdir() if p.is_file())
    print(f'kernel cache {state}: {entry.name} ({size} bytes); purge: kernel/build-base.sh purge', flush=True)


def build(cache, image, platform, environment):
    key = input_key(image, platform, environment)
    entry = cache / key
    if valid(entry, key, environment):
        restore(entry)
        report(entry, 'hit')
        return
    print(f'kernel cache miss: {key}', flush=True)
    with tempfile.TemporaryDirectory(prefix='.build-', dir=cache) as scratch:
        pending = Path(scratch) / 'base'
        subprocess.run(['sh', str(ROOT / 'kernel/build-base.sh'), '_build', str(pending)], check=True)
        current_image = build_env.resolve('kernel', platform)
        if input_key(current_image, platform, environment) != key:
            raise ValueError('kernel inputs changed during build; retry')
        receipt = {'key': key, 'outputs': output_hashes(pending)}
        (pending / RECEIPT).write_text(json.dumps(receipt, sort_keys=True) + '\n')
        if not valid(pending, key, environment):
            raise ValueError('kernel build did not produce all required outputs')
        if entry.exists():
            shutil.rmtree(entry)
        pending.rename(entry)
    restore(entry)
    report(entry, 'stored')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=('build', 'verify', 'key', 'cache-dir', 'purge'))
    command = parser.parse_args().command
    cache = cache_dir()
    if command == 'cache-dir':
        print(cache)
        return
    platform = environment = image = None
    if command != 'purge':
        platform = build_env.platform_for('kernel')
        environment = build_env.identity('kernel', platform)[2]
        image = (build_env.prepare if command == 'build' else build_env.resolve)('kernel', platform)
    if command in ('verify', 'key'):
        key = input_key(image, platform, environment)
        if command == 'key':
            print(key)
        elif not valid(ROOT / 'kernel/out/base', key, environment):
            raise ValueError('kernel outputs are stale or incomplete; run kernel/build-base.sh')
        return
    cache.mkdir(parents=True, exist_ok=True)
    # One lock covers builds and purges across worktrees; the lock inode survives purge.
    with (cache / '.lock').open('a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if command == 'purge':
            for entry in cache.iterdir():
                if entry.is_dir():
                    shutil.rmtree(entry)
            print('kernel cache purged')
        else:
            build(cache, image, platform, environment)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        sys.exit(f'kernel cache failed: {error}')
