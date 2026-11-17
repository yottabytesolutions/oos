# SPDX-License-Identifier: GPL-2.0-or-later
"""Fail-closed image applicability from authoritative base and both sides of Git changes."""
import argparse
import json
import os
from pathlib import Path, PurePosixPath
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
IMAGE_PREFIXES = ('boot/', 'kernel/', 'rootfs/', 'gpu/', '.cargo/', 'xtask/', 'tools/hw/',
                  'tools/build-env/', '.github/workflows/', '.githooks/')
IMAGE_FILES = {'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.python-version', 'ui/build.sh',
               'tools/image-manifest.py', 'tools/bootstrap.sh', 'tools/container-images.json',
               'tools/build-tools.json', 'tools/script_syntax.py', 'tools/check_inventory.py',
               'tools/image_required.py', 'tools/image_suite.py', 'tools/image_validate.py'}
HOST_FILES = {'.gitignore', '.gitattributes', '.aider.conf.yml', 'rustfmt.toml', 'clippy.toml',
              'LICENSE', 'COPYING', 'NOTICE', '.github/CODEOWNERS', '.cursor/rules/oos.mdc',
              'deny.toml', 'cliff.toml'}
# Repository process files outside the workflows: templates, labels, rulesets, Dependabot.
HOST_PREFIXES = ('.github/', 'tools/ci/')


def classify(name):
    path = PurePosixPath(name)
    if path.suffix in {'.md', '.rst'}:
        return False
    if name.startswith(IMAGE_PREFIXES) or name in IMAGE_FILES or path.name in {'Cargo.toml', 'build.rs'}:
        return True
    if name.startswith(HOST_PREFIXES) or name in HOST_FILES:
        return False
    if name.startswith(('tools/output.', 'tools/build-', 'tools/build_', 'tools/')) and not name.startswith(('tools/tests/', 'tools/privacy/')):
        return True
    if name.endswith(('.md', '.rst')) or name in HOST_FILES:
        return False
    if name.startswith(('hal/', 'dsp/', 'ui/', 'fpgad/')):
        if path.suffix in {'.rs', '.txt', '.ppm', '.csv'} and '/assets/' not in name:
            return False
        return True
    if name.startswith(('tools/tests/', 'tools/privacy/')):
        return False
    raise ValueError(f'unclassified build input: {name}; classify it explicitly')


def git(root, *args):
    return subprocess.check_output(['git', *args], cwd=root)


def changed_paths(raw):
    if raw and not raw.endswith(b'\0'):
        raise ValueError('non-NUL change inventory')
    fields = raw.split(b'\0')[:-1]
    paths, i = set(), 0
    while i < len(fields):
        status = fields[i].decode('ascii'); i += 1
        count = 2 if status.startswith(('R', 'C')) else 1
        if not status or status[0] not in 'ACDMRTUXB' or i + count > len(fields):
            raise ValueError('malformed Git change inventory')
        paths.update(os.fsdecode(p) for p in fields[i:i + count]); i += count
    return paths


def base_revision(root, base=None, ci=False):
    if not base:
        if ci or os.environ.get('GITHUB_ACTIONS') == 'true':
            base = os.environ.get('PR_BASE') or os.environ.get('PUSH_BASE')
            if not base or set(base) == {'0'}:
                raise ValueError('missing authoritative PR/push base')
        else:
            refs = git(root, 'for-each-ref', '--format=%(refname)', 'refs/heads/main', 'refs/remotes/origin/main').decode().splitlines()
            if not refs:
                raise ValueError('missing protected base; fetch origin main')
            base = git(root, 'merge-base', 'HEAD', refs[0]).decode().strip()
    git(root, 'cat-file', '-e', base + '^{commit}')
    return base


def required(root, base=None, ci=False):
    base = base_revision(root, base, ci)
    paths = changed_paths(git(root, 'diff', '--name-status', '-z', '--find-renames', base, 'HEAD'))
    # Local candidates include staged, unstaged and untracked sources, without requiring a commit.
    paths |= changed_paths(git(root, 'diff', '--name-status', '-z', '--find-renames', 'HEAD'))
    paths |= {os.fsdecode(p) for p in git(root, 'ls-files', '-z', '--others', '--exclude-standard').split(b'\0') if p}
    decisions = [(path, classify(path)) for path in sorted(paths)]
    return {'required': any(value for _, value in decisions), 'base': base,
            'inputs': [path for path, value in decisions if value]}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--base')
    parser.add_argument('--ci', action='store_true')
    parser.add_argument('--json', action='store_true')
    args = parser.parse_args()
    result = required(ROOT, args.base, args.ci)
    if args.json:
        print(json.dumps(result))
    else:
        print('image-required: ' + ('required (coordinator --image tier)' if result['required'] else 'not applicable'))
    if os.environ.get('GITHUB_OUTPUT'):
        with open(os.environ['GITHUB_OUTPUT'], 'a') as stream:
            stream.write('required=' + str(result['required']).lower() + '\n')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as exc:
        sys.exit(f'image-required: {exc}')
