#!/usr/bin/env python3
"""Render the Cargo-provided inventory twice; compare read-only unless --update."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

from golden import compare_rgb, read_png, require_captures, write_png, write_report

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import output


def child_environment(run):
    env = {'LC_ALL': 'C', 'TZ': 'UTC', 'HOME': str(run / 'home')}
    # Coverage is instrumentation, not an inherited application setting. Keep child profiles discoverable.
    if 'LLVM_PROFILE_FILE' in os.environ:
        env['LLVM_PROFILE_FILE'] = os.environ['LLVM_PROFILE_FILE']
    return env


def child(binary, script, kind, run):
    run.mkdir(parents=True)
    (run / 'store').mkdir()
    (run / 'storage').mkdir()
    args = [str(binary), '--headless-script', '--script', str(script), '--store-dir', 'store']
    if kind != 'demo':
        args += ['--scripted-batch', kind]
    with (run / 'stdout.log').open('wb') as stdout, (run / 'stderr.log').open('wb') as stderr:
        try:
            result = subprocess.run(args, cwd=run, env=child_environment(run),
                                    stdout=stdout, stderr=stderr, timeout=180)
        except subprocess.TimeoutExpired as exc:
            raise ValueError(f'child timeout; logs: {run}') from exc
    errors = (run / 'stderr.log').read_text()
    if result.returncode or any(not s.startswith(('oos-ui pacing:', 'oos-ui shadow:')) for s in errors.splitlines()):
        raise ValueError(f'child status {result.returncode}; diagnostics: {errors}; logs: {run}')


def metadata(path):
    lines = path.read_text().splitlines()
    if len(lines) < 2 or not lines[-1].startswith('settings '):
        raise ValueError(f'missing capture metadata: {path}')
    regions = []
    for line in lines[:-1]:
        name, *coords = line.split()
        if len(coords) != 4:
            raise ValueError('invalid region metadata')
        regions.append((name, *map(int, coords)))
    return regions, lines[-1]


def load_inventory(path):
    rows = [line.split('\t') for line in path.read_text().splitlines()]
    if not rows or any(len(row) != 5 for row in rows):
        raise ValueError('empty or invalid inventory')
    names = [row[0] for row in rows]
    if len(names) != len(set(names)):
        raise ValueError('duplicate cases')
    for name, _, _, shots, _ in rows:
        if Path(name).is_absolute() or '..' in Path(name).parts or not shots:
            raise ValueError('invalid case or zero captures')
    return rows


def capture_case(row, args, out, candidate):
    name, kind, script, shots, provenance = row
    shots = shots.split(',')
    script = Path(script)
    case = out / name
    for repeat in ('one', 'two'):
        child(args.binary, script, kind, case / repeat)
        require_captures(case / repeat, shots)
    result, failures = [], []
    routes, route, decode = {}, 'Demo records; acq.pipeline=batch', 'Demo records'
    for line in (case / 'one/stdout.log').read_text().splitlines():
        if line.startswith('batch '):
            route, decode = line.rsplit(' route ', 1)[-1].split(' decode ')
        elif line.startswith('shot '):
            routes[line[5:]] = (route, decode)
    for shot in shots:
        a, b = (read_png(case / r / shot) for r in ('one', 'two'))
        if a != b:
            raise ValueError(f'nondeterministic fresh-process capture: {name}/{shot}; {case}')
        if a[:2] != (1024, 600):
            raise ValueError('panel size changed')
        meta_a = (case / 'one' / (shot + '.meta')).read_text()
        if meta_a != (case / 'two' / (shot + '.meta')).read_text():
            raise ValueError(f'nondeterministic metadata: {name}/{shot}; {case}')
        regions, settings = metadata(case / 'one' / (shot + '.meta'))
        if (kind != 'demo' or name.startswith('demo/matrix/')) and 'menu_open=false' not in settings:
            raise ValueError(f'matrix capture obscured by a menu: {name}/{shot}')
        expected_path = ROOT / 'ui/tests/golden' / name / shot
        expected = read_png(expected_path) if expected_path.exists() or not args.update else a
        metrics = compare_rgb(expected, a, regions)
        if not args.update and any(not m['pass'] for m in metrics.values()):
            write_report(case / Path(shot).stem, expected, a, metrics)
            failures.append(f'{name}/{shot}: {case / Path(shot).stem / "index.html"}')
        if args.update:
            dest = candidate / name / shot
            dest.parent.mkdir(parents=True, exist_ok=True)
            write_png(dest, a)
            dest.with_suffix('.meta').write_text(meta_a)
        result.append(dict(path=f'{name}/{shot}', rgb_sha256=hashlib.sha256(a[2]).hexdigest(),
                           width=a[0], height=a[1], regions=regions, settings=settings,
                           script_sha256=hashlib.sha256(script.read_bytes()).hexdigest(), provenance=provenance,
                           source_route=routes[shot][0], decode_route=routes[shot][1]))
    return result, failures


def compare_inventory(captures, reference, subset=False):
    expected = {str(p.relative_to(reference)) for p in reference.rglob('*.png')}
    actual = {c['path'] for c in captures}
    if (not subset and actual != expected) or (subset and not actual <= expected):
        raise ValueError(f'reference inventory mismatch: missing={actual - expected}, extra={expected - actual}')


def manifest(captures):
    return dict(format='m1-rgb-v1', fixture='m1-v1', renderer='CPU; 1024x600; bundled fonts',
                reference_base='2b5d170fd59d487bbf8851eab84820b9bd07a24c', hermetic_inputs='4b505b411c99f69ed8e834fbdd5e6bfc68ea8521 (task-0030 integration)',
                seed='0x9e3779b97f4a7c15', schedule=[27, 3, 13, 3, 27, 13],
                clock=1762862400, dt='1/30', tolerance='RGB 1; over-1 pixels <= floor(N/1000) per region',
                scope='synthetic host characterisation; no hardware verification; Demo acq.pipeline=batch',
                captures=captures)


def publish(candidate, reference):
    # Every case has rendered twice before any reference is replaced. Roll back a failed rename.
    pending = reference.with_name('golden.pending')
    backup = reference.with_name('golden.previous')
    if pending.exists() or backup.exists():
        raise ValueError('unfinished reference transaction; preserve and inspect it')
    shutil.copytree(candidate, pending)
    if (reference / 'README.md').exists():
        shutil.copyfile(reference / 'README.md', pending / 'README.md')
    if reference.exists():
        reference.rename(backup)
    try:
        pending.rename(reference)
    except BaseException:
        if backup.exists():
            backup.rename(reference)
        raise
    if backup.exists():
        shutil.rmtree(backup)


def prune_runs(base, keep=2):
    runs = sorted((p for p in base.glob('run-*') if p.is_dir()),
                  key=lambda p: p.stat().st_mtime_ns, reverse=True)
    for old in runs[keep:]:
        shutil.rmtree(old)


def run_base(root, profile):
    # Instrumented builds can have debug assertions too; they must never prune debug runs.
    name = f'coverage-{profile}' if 'LLVM_PROFILE_FILE' in os.environ else profile
    return root / 'golden' / name


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--inventory', type=Path, required=True)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--profile', choices=['debug', 'release'], required=True)
    parser.add_argument('--update', action='store_true')
    parser.add_argument('--subset', action='store_true')
    parser.add_argument('--output-root', type=Path)
    args = parser.parse_args()
    if args.update and (args.subset or args.profile != 'release'):
        raise ValueError('updates require the full release inventory')
    suite = unittest.defaultTestLoader.discover(str(Path(__file__).parent), pattern='test_golden.py')
    if not unittest.TextTestRunner().run(suite).wasSuccessful():
        raise ValueError('PNG comparator self-tests failed')
    rows = load_inventory(args.inventory)
    base = run_base(args.output_root or output.target(), args.profile)
    base.mkdir(parents=True, exist_ok=True)
    prune_runs(base, keep=1)
    out = Path(tempfile.mkdtemp(prefix='run-', dir=base))
    print(f'review artifacts: {out}', flush=True)
    candidate = out / 'candidates'
    reference = ROOT / 'ui/tests/golden'
    start = time.monotonic()
    captures, failures = [], []
    for index, row in enumerate(rows):
        added, failed = capture_case(row, args, out, candidate)
        captures.extend(added)
        failures.extend(failed)
        print(f'{index + 1}/{len(rows)} {row[0]}: {len(added)} captures', flush=True)
    by_path = {c['path']: c for c in captures}
    for capture in captures:
        if '/Auto-' in capture['path']:
            other = by_path.get(capture['path'].replace('/Auto-', '/Off-'))
            if other is None or capture['rgb_sha256'] != other['rgb_sha256']:
                raise ValueError(f'Auto does not equal current lines: {capture["path"]}')
    record = manifest(captures)
    elapsed = time.monotonic() - start
    (out / 'summary.json').write_text(json.dumps(dict(cases=len(rows), captures=len(captures),
        elapsed_seconds=elapsed, failures=failures)) + '\n')
    if failures:
        (out / 'report.json').write_text(json.dumps(record, indent=2) + '\n')
        (out / 'index.html').write_text('<!doctype html><title>M1 failures</title>' + ''.join(
            f'<p>{failure}</p>' for failure in failures))
        raise ValueError('\n'.join(failures))
    if args.update:
        (candidate / 'manifest.txt').write_text(json.dumps(record, indent=2, sort_keys=True) + '\n')
        publish(candidate, reference)
    else:
        compare_inventory(captures, reference, args.subset)
        previous = {c['path']: c for c in json.loads((reference / 'manifest.txt').read_text())['captures']}
        # Reference hashes describe the baseline; ordinary comparisons use the documented tolerance.
        for new in captures:
            old = previous[new['path']]
            for key in ('path', 'regions', 'settings', 'script_sha256', 'provenance', 'source_route', 'decode_route'):
                if old[key] != json.loads(json.dumps(new[key])):
                    raise ValueError(f'capture contract changed: {new["path"]} {key}; explicit update required')
    print(f'elapsed_seconds={elapsed:.3f} profile={args.profile} subset={args.subset}')
    print(f'PASS: {len(rows)} cases, {2 * len(rows)} fresh processes, {2 * len(captures)} exact-repeat captures; {out}')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError) as exc:
        sys.exit(f'golden: {exc}')
