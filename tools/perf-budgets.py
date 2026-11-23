#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Check host observations against the architecture's measurable budget rows."""
import argparse
import math
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
ALLOCATIONS = {'deposit_spans', 'deposit_dots', 'deposit_codes', 'cheap_measurements',
               'core_matching_learn_and_decode', 'core_matching_worker_decode'}
WORK_CASES = {'DMA_READ_WORK': 5, 'SPAN_WORK': 18, 'DOT_WORK': 9, 'UPLOAD_WORK': 2, 'BATCH_PICTURE_WORK': 2}


def budgets(text):
    section = text.split('### Host budget rows\n', 1)[1].split('### How each budget is measured', 1)[0]
    rows = {}
    for line in section.splitlines():
        if not line.startswith('| ') or line.startswith('| Guard ID '):
            continue
        fields = [field.strip() for field in line.strip('|').split('|')]
        if len(fields) != 5 or fields[0] in rows:
            raise ValueError('malformed or duplicate host budget row')
        best, tail = map(float, fields[1:3])
        if not all(math.isfinite(value) and value > 0 for value in (best, tail)) or tail < best:
            raise ValueError('invalid host ratio limit')
        rows[fields[0]] = (best, tail)
    if not rows:
        raise ValueError('no host budget rows')
    return rows


def check(rows, log):
    seen, allocations = set(), set()
    work = dict.fromkeys(WORK_CASES, 0)
    for line in log.splitlines():
        # libtest may prefix an observation with its test name without a newline.
        if 'CPU_BUDGET ' in line:
            fields = line.split('CPU_BUDGET ', 1)[1].split()
            name, *values = fields
            if name not in rows or len(values) != 4:
                raise ValueError('unknown or malformed CPU observation')
            a, b, p, q = map(int, values)
            if min(a, b, p, q) <= 0:
                raise ValueError('invalid CPU distribution')
            best, tail = rows[name]
            if b / a > best or q / p > tail:
                raise ValueError(f'{name}: best {b/a:.3f} <= {best}, p99 {q/p:.3f} <= {tail} failed')
            seen.add(name)
        for marker in work:
            if f'{marker} ' in line:
                work[marker] += 1
        if 'ALLOC_BUDGET ' in line:
            name, frames, count = line.split('ALLOC_BUDGET ', 1)[1].split()
            if name not in ALLOCATIONS or int(frames) < 64 or int(count) != 0:
                raise ValueError('invalid allocation observation')
            allocations.add(name)
    if seen != set(rows) or allocations != ALLOCATIONS:
        raise ValueError(f'missing observations: cpu={sorted(set(rows)-seen)}, allocations={sorted(ALLOCATIONS-allocations)}')
    if work != WORK_CASES:
        raise ValueError(f'expected deterministic work observations {WORK_CASES}, got {work}')
    return (f'{len(seen)} CPU ratio budgets, {len(allocations)} zero-allocation budgets and '
            f'{sum(work.values())} bounded-work cases passed')


def sampler_copies_match(root):
    paths = ['ui/src/system/perfguard.rs', 'hal/tests/perf_support/samples.rs']
    if (root / paths[0]).read_bytes() != (root / paths[1]).read_bytes():
        raise ValueError('HAL/UI timing samplers differ')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--log', type=Path, help='combined perf-guard output; without this, list the host budgets')
    args = parser.parse_args()
    rows = budgets((ROOT / 'docs/architecture/README.md').read_text())
    sampler_copies_match(ROOT)
    if args.log:
        print(check(rows, args.log.read_text()))
    else:
        for name, (best, tail) in rows.items():
            print(f'{name}: best CPU <= {best}, p99 CPU <= {tail}')
        print('unit absolute budgets: not measured on the host')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, IndexError, OSError) as error:
        sys.exit(f'perf budgets: {error}')
