#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Validate the versioned references or stage just those captures using the golden runner."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

from golden import read_png
from golden_run import capture_case, load_inventory

REFERENCE = Path('ui/tests/golden')


def references():
    manifest = json.loads((REFERENCE / 'manifest.txt').read_text())
    versions = {}
    for capture in manifest['captures']:
        if 'release_version' not in capture:
            continue
        name = capture['path']
        path = Path(name)
        if path.is_absolute() or '..' in path.parts or path.suffix != '.png' or name in versions:
            raise ValueError(f'invalid release capture path: {name}')
        versions[name] = capture
    if not versions:
        raise ValueError('manifest.txt: no release version captures')
    return manifest, versions


def check(versions, version):
    errors = []
    for name, capture in versions.items():
        path = REFERENCE / name
        if capture['release_version'] != version:
            errors.append(f'{path}: manifest.txt records release {capture["release_version"]}; expected {version}')
        try:
            digest = hashlib.sha256(read_png(path)[2]).hexdigest()
            if digest != capture.get('rgb_sha256'):
                errors.append(f'{path}: decoded RGB differs from manifest.txt rgb_sha256')
        except (ValueError, OSError) as error:
            errors.append(f'{path}: {error}')
    return errors


def select(row, versions, stage):
    name, kind, script, shots, provenance = row
    selected = {shot for shot in shots.split(',') if f'{name}/{shot}' in versions}
    if not selected:
        return None
    target = stage / 'scripts' / name / 'script.txt'
    target.parent.mkdir(parents=True)
    # A shot advances one frame. Keep that step for the surrounding script.
    lines = Path(script).read_text().splitlines()
    target.write_text(''.join(
        ('frames 1' if line.startswith('shot ') and line[5:] not in selected else line) + '\n'
        for line in lines
    ))
    return name, kind, str(target), ','.join(sorted(selected)), provenance


def render(args, manifest, versions):
    rendered = {}
    args.update = True
    candidate = args.stage / 'references'
    for row in load_inventory(args.inventory):
        selected = select(row, versions, args.stage)
        if selected:
            captures, _ = capture_case(selected, args, args.stage / 'runs', candidate)
            rendered.update((capture['path'], capture) for capture in captures)
    if rendered.keys() != versions.keys():
        raise ValueError(f'release captures missing from inventory: {sorted(versions.keys() - rendered.keys())}')
    for name, capture in rendered.items():
        if capture.get('release_version') != args.version:
            raise ValueError(f'{name}: expected to render release {args.version}')
        versions[name].update(release_version=args.version, rgb_sha256=capture['rgb_sha256'])
    (candidate / 'manifest.txt').write_text(json.dumps(manifest, indent=2, sort_keys=True) + '\n')
    return sorted(rendered)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('version')
    parser.add_argument('--stage', type=Path)
    parser.add_argument('--inventory', type=Path)
    parser.add_argument('--binary', type=Path)
    args = parser.parse_args()
    manifest, versions = references()
    if args.stage:
        for name in render(args, manifest, versions):
            print(name)
    else:
        errors = check(versions, args.version)
        if errors:
            sys.exit('\n'.join(errors))


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError) as error:
        sys.exit(f'release goldens: {error}')
