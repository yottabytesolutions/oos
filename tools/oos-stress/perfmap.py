#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Read-only P0 windows: perfmap.py --host example.invalid --out /path/to/results A-1.

Prepare each setup separately with the task 0085 SCPI sequences. Positional names
label successive windows of that current setup, without applying any commands.
Requires an existing observe/test lease; only lease bookkeeping renews on device.
"""
from __future__ import annotations

import argparse
from collections.abc import Callable
import json
import math
import os
from pathlib import Path
import re
import socket
import subprocess
import sys
import time
from typing import cast

HERE = Path(__file__).resolve().parent
sys.path[:0] = [str(HERE), str(HERE.parent)]
import scope_lease
from perfmap_parse import capture_delta, parse_capture, parse_state
from perfmap_types import DRAW_NAMES, Measurement, ProcessDelta, Snapshot, State

# Fixed commands only. Optional files are framed even when unavailable. No device
# file, clock policy, register, process affinity or performance setting is written.
CAPTURE = r'''
set -eu
section() { printf '@@ %s\n' "$1"; }
readfile() { section "$1"; cat "$2" 2>/dev/null || :; printf '\n'; }
ui_pids=$(pidof oos-ui oos-ui.new || :)
set -- $ui_pids
[ "$#" -eq 1 ] || { printf 'expected one oos-ui process\n' >&2; exit 1; }
ui_pid=$1
readfile ui_stat "/proc/$ui_pid/stat"
for task_dir in /proc/"$ui_pid"/task/*; do
    tid=${task_dir##*/}
    readfile "thread_stat:$tid" "$task_dir/stat"
    readfile "thread_status:$tid" "$task_dir/status"
done
gpu_pids=$(pidof oos-gpu || :)
set -- $gpu_pids
[ "$#" -le 1 ] || { printf 'ambiguous oos-gpu process\n' >&2; exit 1; }
readfile gpu_stat "/proc/${1:-unavailable}/stat"
readfile interrupts /proc/interrupts
readfile policy0 /sys/devices/system/cpu/cpufreq/policy0/stats/time_in_state
readfile policy4 /sys/devices/system/cpu/cpufreq/policy4/stats/time_in_state
section gpu_trans_stat
for gpu_path in /sys/class/devfreq/*gpu*/trans_stat; do
    if [ -r "$gpu_path" ]; then cat "$gpu_path"; break; fi
done
printf '\n'
section clock_ticks
getconf CLK_TCK 2>/dev/null || :
printf '\n'
readfile ui_stat_end "/proc/$ui_pid/stat"
'''


def ssh(host: str, script: str) -> str:
    """device-test SSH trust/identity options, with scope_lease's sh -s transport."""
    args = ['ssh', '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=yes',
            '-o', 'ConnectTimeout=10', '-p', os.environ.get('OOS_SSH_PORT', '22')]
    identity = os.environ.get('OOS_SSH_IDENTITY')
    if identity:
        args += ['-i', identity]
    try:
        result = subprocess.run(args + [f'root@{host}', 'sh -s'], input=script,
                                text=True, capture_output=True, timeout=30, check=True)
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as exc:
        detail = exc.stderr or '(empty stderr)'
        if isinstance(detail, bytes):
            detail = detail.decode('utf-8', errors='replace')
        reason = f'exited {exc.returncode}' if isinstance(exc, subprocess.CalledProcessError) else 'timed out'
        raise ValueError(f'SSH {reason}: {detail.strip()}; stop') from exc
    return result.stdout


class Scpi:
    """Exactly one socket, bounded replies, at least 150 ms between requests."""
    def __init__(self, sock: socket.socket, lease: scope_lease.HeldLease, *,
                 monotonic: Callable[[], float] = time.monotonic,
                 sleep: Callable[[float], None] = time.sleep) -> None:
        self.sock, self.lease = sock, lease
        self.monotonic, self._sleep = monotonic, sleep
        self.reader = sock.makefile('rb')
        self.last_sent = -math.inf

    def wait_until(self, deadline: float) -> None:
        remaining = deadline - self.monotonic()
        if remaining > 0:
            self._sleep(remaining)

    def state(self) -> State:
        self.wait_until(self.last_sent + 0.150)
        self.lease.check()
        started = self.monotonic()
        self.sock.sendall(b':SYSTem:STATe?\n')
        self.last_sent = started
        raw = self.reader.readline(1024 * 1024)
        if not raw.endswith(b'\n'):
            raise ValueError('closed connection or oversized/incomplete STATe reply; stop')
        text = raw.decode('utf-8').strip()
        diagnostics = parse_state(text)
        if (not text.startswith('fps ')
                or any(len(diagnostics.get(f'draw.{name}', [])) != 1 for name in DRAW_NAMES)):
            raise ValueError('unexpected STATe reply; stop')
        return {'started_s': started, 'finished_s': self.monotonic(),
                'raw': text, 'diagnostics': diagnostics}

    def close(self) -> None:
        self.reader.close()


def snapshot(scpi: Scpi, host: str) -> Snapshot:
    state = scpi.state()
    scpi.lease.check()
    started = scpi.monotonic()
    proc = parse_capture(ssh(host, CAPTURE))
    scpi.lease.check()
    return {'state': state, 'proc_started_s': started, 'proc_finished_s': scpi.monotonic(),
            'proc': proc}


def measure(scpi: Scpi, host: str, name: str, seconds: float) -> Measurement:
    first = snapshot(scpi, host)
    scpi.wait_until(first['state']['started_s'] + seconds)
    second = snapshot(scpi, host)
    return {'schema': 1, 'point': name, 'requested_seconds': seconds,
            'recorded_at': time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime()),
            'state_interval_s': second['state']['started_s'] - first['state']['started_s'],
            'proc_interval_s': second['proc_started_s'] - first['proc_started_s'],
            'before': first, 'after': second, 'delta': capture_delta(first['proc'], second['proc'])}


def markdown(rows: list[Measurement]) -> str:
    names = DRAW_NAMES
    header = '| Point | proc s | UI ticks/s | GPU ticks/s | xdma IRQ/s | ' + ' | '.join(
        f'{name} p50/p99 ms' for name in names) + ' | Missing/reset |\n'
    result = header + '|---|' + '---:|' * (4 + len(names)) + '---|\n'
    for row in rows:
        elapsed = row['proc_interval_s']
        threads = list(row['delta']['threads'].values())
        available = [cast(ProcessDelta, t) for t in threads if 'unavailable' not in t]
        # /proc/PID/stat includes CPU from scoped helpers reaped between samples.
        ui = row['delta']['ui']
        gpu = row['delta']['gpu']
        irq = list(row['delta']['interrupts'].values())
        def rate(values: list[int | None]) -> str:
            if not values or any(v is None for v in values):
                return 'n/a'
            return f'{sum(v for v in values if v is not None) / elapsed:.2f}'
        cells = [row['point'], f'{elapsed:.3f}', rate([ui['counters']['cpu_ticks']]),
                 rate([gpu['counters']['cpu_ticks']]) if gpu else 'n/a', rate(irq)]
        diagnostics = row['after']['state']['diagnostics']
        absent = []
        for name in names:
            blocks = diagnostics.get(f'draw.{name}', [])
            fields = blocks[0] if len(blocks) == 1 else {}
            if not fields:
                absent.append(f'draw.{name}')
            values = [fields.get(f'p{p}_ns', 'missing') for p in [50, 99]]
            cells.append('/'.join(f'{v / 1e6:.3f}' if isinstance(v, int) else str(v) for v in values))
        missing = row['before']['proc']['missing'] + row['after']['proc']['missing'] + absent
        delta = row['delta']
        invalid = (not ui['same_identity'] or None in ui['counters'].values()
                   or any(not t['same_identity'] or None in t['counters'].values() for t in available)
                   or any(None in counters.values() for counters in
                          [delta['interrupts'], delta['policy0'], delta['policy4'], delta['gpu_trans_stat']])
                   or (gpu is not None and (not gpu['same_identity'] or None in gpu['counters'].values())))
        cells.append(str(len(missing)) + ('; reset/churn' if invalid else ''))
        result += '| ' + ' | '.join(cells) + ' |\n'
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('points', nargs='+', help='unique window names; no setup commands are sent')
    parser.add_argument('--host', help='otherwise OOS_SCOPE; no default')
    parser.add_argument('--port', type=int, default=5555)
    parser.add_argument('--seconds', type=float, default=20)
    parser.add_argument('--settle', type=float, default=12, help='seconds before each window')
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    if (not math.isfinite(args.seconds) or args.seconds <= 0
            or not math.isfinite(args.settle) or args.settle < 0):
        parser.error('seconds must be finite and positive; settle finite and nonnegative')
    if (len(set(args.points)) != len(args.points)
            or any(not re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,95}', name) for name in args.points)):
        parser.error('point names must be unique plain identifiers, at most 96 characters')
    args.out.mkdir(parents=True, exist_ok=True)
    if any((args.out / f'{name}.json').exists() for name in args.points):
        parser.error('point JSON already exists; use new names or output directory')
    host = scope_lease.host_value(args.host)
    rows: list[Measurement] = [json.loads(path.read_text()) for path in sorted(args.out.glob('*.json'))]
    if any(not isinstance(row, dict) or row.get('schema') != 1 or 'point' not in row for row in rows):
        parser.error('output directory contains unrelated JSON; use a dedicated directory')
    with scope_lease.held(host, cap='observe', executor=ssh) as lease:
        lease.check()
        with socket.create_connection((host, args.port), timeout=15) as sock:
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            scpi = Scpi(sock, lease)
            try:
                for name in args.points:
                    if args.settle > 0:
                        time.sleep(args.settle)
                    row = measure(scpi, host, name, args.seconds)
                    with (args.out / f'{name}.json').open('x') as output:
                        output.write(json.dumps(row, indent=2) + '\n')
                    rows.append(row)
                    (args.out / 'report.md').write_text(markdown(rows))
                    print(f'{name}: captured', flush=True)
            finally:
                scpi.close()
    print(markdown(rows))
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, subprocess.SubprocessError, scope_lease.LeaseError) as exc:
        sys.exit(f'perfmap: {exc}')
