# SPDX-License-Identifier: GPL-2.0-or-later
"""Pure Linux/STATe parsers for perfmap; no device access or third-party modules."""
from __future__ import annotations

import re
from typing import Callable, Literal, TypeVar

from perfmap_types import (
    Capture, ContextSwitches, CounterDelta, Counters, Delta, Diagnostic, Diagnostics,
    Process, ProcessDelta, Unavailable,
)

Parsed = TypeVar('Parsed')


def parse_stat(text: str) -> Process:
    """Parse /proc/PID[/task/TID]/stat, including spaces and ')' in comm."""
    left, right = text.find('('), text.rfind(')')
    fields = text[right + 1:].split()
    if left < 1 or right <= left or len(fields) < 37:
        raise ValueError('truncated proc stat')
    return {'pid': int(text[:left]), 'comm': text[left + 1:right],
            'minflt': int(fields[7]), 'cpu_ticks': int(fields[11]) + int(fields[12]),
            'starttime': int(fields[19]), 'processor': int(fields[36])}


def parse_status(text: str) -> ContextSwitches:
    labels = {'voluntary_ctxt_switches', 'nonvoluntary_ctxt_switches'}
    rows = dict(line.split(':', 1) for line in text.splitlines() if ':' in line)
    if not labels <= rows.keys():
        raise ValueError('missing context-switch counters')
    return {'voluntary_ctxt_switches': int(rows['voluntary_ctxt_switches']),
            'nonvoluntary_ctxt_switches': int(rows['nonvoluntary_ctxt_switches'])}


def parse_interrupts(text: str) -> dict[str, int]:
    lines = text.splitlines()
    if not lines:
        raise ValueError('missing interrupt header')
    cpus = lines[0].split()
    if not cpus or any(not re.fullmatch(r'CPU\d+', cpu) for cpu in cpus):
        raise ValueError('invalid interrupt header')
    rows = {}
    for line in lines[1:]:
        if 'xdma' not in line.lower():
            continue
        irq, rest = line.split(':', 1)
        values = rest.split()
        if len(values) < len(cpus) + 1:
            raise ValueError('truncated xdma counters')
        for cpu, value in zip(cpus, values):
            rows[f'{irq.strip()}:{cpu}'] = int(value)
    return rows


def parse_time_in_state(text: str) -> dict[str, int]:
    rows = {}
    for line in text.splitlines():
        if line.strip():
            frequency, ticks = line.split()
            rows[str(int(frequency))] = int(ticks)
    if not rows:
        raise ValueError('empty time_in_state')
    return rows


def parse_trans_stat(text: str) -> dict[str, int]:
    """Keep transitions and time(ms) residency separate, keyed by frequency."""
    columns: list[str] = []
    result = {}
    for line in text.splitlines():
        if ':' not in line:
            continue
        label, values = line.split(':', 1)
        if not label.strip() and 'time(ms)' in values:
            columns = re.findall(r'\d+', values)
        elif re.fullmatch(r'\s*\*?\s*\d+\s*', label):
            frequency = str(int(label.replace('*', '').strip()))
            counts = values.split()
            if not columns or len(counts) != len(columns) + 1:
                raise ValueError('invalid devfreq transition row')
            for target, count in zip(columns, counts):
                result[f'transition:{frequency}->{target}'] = int(count)
            result[f'time_ms:{frequency}'] = int(counts[-1])
        elif label.strip().lower() == 'total transition':
            result['total_transitions'] = int(values)
    if not any(key.startswith('time_ms:') for key in result):
        raise ValueError('missing devfreq transition matrix (disabled or unsupported)')
    return result


def parse_state(text: str) -> Diagnostics:
    """Keep every diagnostic block per label; stop at its first non-key=value token."""
    rows: Diagnostics = {}
    for block in text.split(' diag ')[1:]:
        tokens = block.split()
        if not tokens:
            continue
        fields: Diagnostic = {}
        for token in tokens[1:]:
            if '=' not in token:
                break
            key, value = token.split('=', 1)
            fields[key] = int(value) if value.isdecimal() else value
        rows.setdefault(tokens[0], []).append(fields)
    return rows


def counter_delta(before: Counters, after: Counters) -> CounterDelta:
    """Null means absent or reset; never turn restart/wrap into a plausible rate."""
    return {key: after[key] - before[key]
            if key in before and key in after and after[key] >= before[key] else None
            for key in sorted(before.keys() | after.keys())}


def parse_capture(text: str) -> Capture:
    """Sections framed by the fixed read-only SSH script; missing files stay visible."""
    sections: dict[str, str] = {}
    current = None
    for line in text.splitlines():
        if line.startswith('@@ '):
            current = line[3:]
            if current in sections:
                raise ValueError('duplicate capture section')
            sections[current] = ''
        elif current is not None:
            sections[current] += line + '\n'
    missing: list[str] = []
    def optional(key: str, parser: Callable[[str], Parsed]) -> Parsed | None:
        try:
            return parser(sections.get(key, ''))
        except (ValueError, IndexError) as exc:
            missing.append(f'{key}: {exc}')
            return None
    ui = optional('ui_stat', parse_stat)
    if ui is None:
        raise ValueError('oos-ui identity unavailable; stop collection')
    result: Capture = {'threads': {}, 'missing': missing, 'raw': sections,
                       'interrupts': optional('interrupts', parse_interrupts),
                       'policy0': optional('policy0', parse_time_in_state),
                       'policy4': optional('policy4', parse_time_in_state),
                       'gpu_trans_stat': optional('gpu_trans_stat', parse_trans_stat),
                       'gpu_stat': optional('gpu_stat', parse_stat), 'ui_stat': ui,
                       'clock_ticks': None}
    if result['interrupts'] == {}:
        result['missing'].append('interrupts: no xdma line')
    for key, value in sections.items():
        if not key.startswith('thread_stat:'):
            continue
        tid = key.split(':')[1]
        try:
            row = parse_stat(value)
            row.update(parse_status(sections.get(f'thread_status:{tid}', '')))
            result['threads'][tid] = row
        except (ValueError, IndexError) as exc:
            result['missing'].append(f'{key}: {exc}')
    hz = sections.get('clock_ticks', '').strip()
    result['clock_ticks'] = int(hz) if hz.isdecimal() and int(hz) > 0 else None
    if result['clock_ticks'] is None:
        result['missing'].append('clock_ticks: unavailable')
    end = parse_stat(sections.get('ui_stat_end', ''))
    if (end['pid'], end['starttime']) != (result['ui_stat']['pid'], result['ui_stat']['starttime']):
        raise ValueError('oos-ui restarted during capture; stop collection')
    return result


def process_delta(before: Process, after: Process) -> ProcessDelta:
    same = (before['pid'], before['starttime']) == (after['pid'], after['starttime'])
    keys: list[Literal['cpu_ticks', 'minflt', 'voluntary_ctxt_switches', 'nonvoluntary_ctxt_switches']] = [
        'cpu_ticks', 'minflt', 'voluntary_ctxt_switches', 'nonvoluntary_ctxt_switches']
    counters = counter_delta({k: before[k] for k in keys if k in before},
                             {k: after[k] for k in keys if k in after})
    return {'comm': after['comm'], 'same_identity': same,
            'processor_before': before['processor'], 'processor_after': after['processor'],
            'counters': counters if same else dict.fromkeys(counters)}


def capture_delta(before: Capture, after: Capture) -> Delta:
    ui = process_delta(before['ui_stat'], after['ui_stat'])
    same = ui['same_identity']
    transient = before['threads'].keys() ^ after['threads'].keys()
    threads: dict[str, ProcessDelta | Unavailable] = {}
    for tid in sorted(before['threads'].keys() | after['threads'].keys(), key=int):
        if not same:
            threads[tid] = {'unavailable': 'process restarted'}
        elif tid in transient:
            threads[tid] = {'unavailable': 'expected thread birth/exit between snapshots'}
        else:
            threads[tid] = process_delta(before['threads'][tid], after['threads'][tid])
    return {'ui': ui, 'threads': threads, 'expected_thread_churn': len(transient) if same else 0,
            'ui_same_identity': same,
            'interrupts': counter_delta(before['interrupts'] or {}, after['interrupts'] or {}),
            'policy0': counter_delta(before['policy0'] or {}, after['policy0'] or {}),
            'policy4': counter_delta(before['policy4'] or {}, after['policy4'] or {}),
            'gpu_trans_stat': counter_delta(before['gpu_trans_stat'] or {}, after['gpu_trans_stat'] or {}),
            'gpu': (process_delta(before['gpu_stat'], after['gpu_stat'])
                    if before['gpu_stat'] and after['gpu_stat'] else None)}
