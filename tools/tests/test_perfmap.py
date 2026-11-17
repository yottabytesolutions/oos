# SPDX-License-Identifier: GPL-2.0-or-later
"""Host-only P0 fixtures and transport contracts; never connects to a unit."""
import copy
import contextlib
import io
import json
import tempfile
import importlib.util
from pathlib import Path
import socket
import subprocess
import sys
import threading
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'tools/oos-stress'
sys.path.insert(0, str(SOURCE))
import perfmap_parse as parse
SPEC = importlib.util.spec_from_file_location('perfmap', SOURCE / 'perfmap.py')
PERF = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PERF)
FIXTURES = ROOT / 'tools/tests/fixtures/perfmap'


class Perfmap(unittest.TestCase):
    def setUp(self):
        self.before_text = (FIXTURES / 'before.txt').read_text()
        self.after_text = (FIXTURES / 'after.txt').read_text()
        self.before = parse.parse_capture(self.before_text)
        self.after = parse.parse_capture(self.after_text)
        self.state = (FIXTURES / 'state.txt').read_text()

    def test_proc_stat_fields_and_parenthesized_names(self):
        thread = self.after['threads']['101']
        self.assertEqual(thread['comm'], 'draw ) worker')
        self.assertEqual(thread['cpu_ticks'], 140)
        self.assertEqual(thread['minflt'], 8)
        self.assertEqual(thread['processor'], 2)
        self.assertEqual(thread['starttime'], 1335)
        self.assertEqual(thread['voluntary_ctxt_switches'], 15)
        self.assertEqual(thread['nonvoluntary_ctxt_switches'], 22)
        self.assertEqual(self.after['clock_ticks'], 100)

    def test_all_requested_counter_deltas(self):
        delta = parse.capture_delta(self.before, self.after)
        thread = delta['threads']['101']
        self.assertEqual(thread['counters'], {'cpu_ticks': 40, 'minflt': 3,
                         'voluntary_ctxt_switches': 5, 'nonvoluntary_ctxt_switches': 2})
        self.assertEqual((thread['processor_before'], thread['processor_after']), (1, 2))
        self.assertEqual(delta['ui']['counters'], {'cpu_ticks': 40, 'minflt': 3})
        self.assertEqual(delta['expected_thread_churn'], 0)
        self.assertEqual(delta['gpu']['counters'], {'cpu_ticks': 30, 'minflt': 1})
        self.assertEqual(sum(delta['interrupts'].values()), 10)
        self.assertEqual(delta['policy0'], {'408000': 5, '816000': 15})
        self.assertEqual(delta['policy4'], {'816000': 10, '1416000': 10})
        self.assertEqual(delta['gpu_trans_stat']['time_ms:600000000'], 1800)
        self.assertEqual(delta['gpu_trans_stat']['transition:200000000->600000000'], 1)
        self.assertEqual(delta['gpu_trans_stat']['total_transitions'], 2)

    def test_counter_reset_and_missing_are_null(self):
        self.assertEqual(parse.counter_delta({'a': 10, 'c': 2}, {'a': 1, 'b': 2}),
                         {'a': None, 'b': None, 'c': None})
        after = copy.deepcopy(self.after)
        after['threads']['100']['minflt'] = 0
        self.assertIsNone(parse.capture_delta(self.before, after)['threads']['100']['counters']['minflt'])

    def test_thread_churn_and_pid_reuse_cannot_create_cpu_time(self):
        after = copy.deepcopy(self.after)
        after['threads'].pop('100')
        after['threads']['102'] = after['threads'].pop('101')
        delta = parse.capture_delta(self.before, after)
        self.assertTrue(all('unavailable' in row for row in delta['threads'].values()))
        after = copy.deepcopy(self.after)
        after['threads']['101']['starttime'] += 1
        self.assertIsNone(parse.capture_delta(self.before, after)['threads']['101']['counters']['cpu_ticks'])
        after['ui_stat']['starttime'] += 1
        self.assertFalse(parse.capture_delta(self.before, after)['ui_same_identity'])
        after['gpu_stat']['starttime'] += 1
        self.assertIsNone(parse.capture_delta(self.before, after)['gpu']['counters']['cpu_ticks'])

    def test_ui_process_ticks_include_helpers_missing_from_both_snapshots(self):
        after = copy.deepcopy(self.after)
        # The process total also includes helpers reaped between the snapshots.
        after['ui_stat']['cpu_ticks'] = self.before['ui_stat']['cpu_ticks'] + 120
        delta = parse.capture_delta(self.before, after)
        self.assertEqual(delta['ui']['counters']['cpu_ticks'], 120)
        self.assertEqual(sum(t['counters']['cpu_ticks'] for t in delta['threads'].values()), 80)
        row = {'point': 'A-helpers', 'proc_interval_s': 2.0,
               'before': {'proc': self.before},
               'after': {'proc': after, 'state': {'diagnostics': parse.parse_state(self.state)}},
               'delta': delta}
        self.assertIn('60.00 | 15.00 | 5.00', PERF.markdown([row]))

    def test_transient_tid_at_either_endpoint_is_expected_churn(self):
        for endpoint in ['before', 'after']:
            with self.subTest(endpoint=endpoint):
                before, after = copy.deepcopy(self.before), copy.deepcopy(self.after)
                capture = before if endpoint == 'before' else after
                capture['threads']['102'] = dict(capture['threads']['101'], pid=102, comm='helper')
                delta = parse.capture_delta(before, after)
                self.assertEqual(delta['expected_thread_churn'], 1)
                self.assertIn('unavailable', delta['threads']['102'])
                self.assertEqual(delta['threads']['101']['counters']['cpu_ticks'], 40)
                row = {'point': 'A-churn', 'proc_interval_s': 2.0,
                       'before': {'proc': before},
                       'after': {'proc': after, 'state': {'diagnostics': parse.parse_state(self.state)}},
                       'delta': delta}
                report = PERF.markdown([row])
                self.assertIn('20.00 | 15.00 | 5.00', report)
                self.assertNotIn('reset/churn', report)
                saved = json.loads(json.dumps(row))
                self.assertEqual(saved['delta']['expected_thread_churn'], 1)

    def test_ui_reset_or_restart_invalidates_rate_even_with_stable_threads(self):
        for field, value in [('cpu_ticks', 1), ('starttime', 9999)]:
            with self.subTest(field=field):
                after = copy.deepcopy(self.after)
                after['ui_stat'][field] = value
                delta = parse.capture_delta(self.before, after)
                self.assertIsNone(delta['ui']['counters']['cpu_ticks'])
                self.assertEqual(delta['expected_thread_churn'], 0)
                row = {'point': 'A-reset', 'proc_interval_s': 2.0,
                       'before': {'proc': self.before},
                       'after': {'proc': after, 'state': {'diagnostics': parse.parse_state(self.state)}},
                       'delta': delta}
                report = PERF.markdown([row])
                self.assertIn('n/a | 15.00 | 5.00', report)
                self.assertIn('reset/churn', report)

    def test_optional_missing_stats_remain_visible(self):
        text = self.before_text.replace('@@ policy0\n408000 20\n816000 80\n', '@@ policy0\n')
        text = text.replace('@@ clock_ticks\n100\n', '@@ clock_ticks\n')
        capture = parse.parse_capture(text)
        self.assertIsNone(capture['policy0'])
        self.assertIsNone(capture['clock_ticks'])
        self.assertIn('policy0: empty time_in_state', capture['missing'])
        self.assertEqual(parse.parse_interrupts('CPU0 CPU1\n1: 3 4 eth0\n'), {})
        with self.assertRaises(ValueError):
            parse.parse_trans_stat('Not supported\n')

    def test_malformed_required_and_duplicate_sections_fail_closed(self):
        for text in ['', '1 (short) R 1', 'not a stat line']:
            with self.assertRaises(ValueError):
                parse.parse_stat(text)
        with self.assertRaises(ValueError):
            parse.parse_status('Name: worker\n')
        for text in ['', self.before_text + '@@ ui_stat\n']:
            with self.assertRaises(ValueError):
                parse.parse_capture(text)
        text = self.before_text.replace('@@ ui_stat_end\n100 (oos-ui)', '@@ ui_stat_end\n999 (oos-ui)')
        with self.assertRaises(ValueError):
            parse.parse_capture(text)

    def test_state_pending_zero_and_markdown_units(self):
        diagnostics = parse.parse_state(self.state)
        self.assertTrue(self.state.startswith('fps '))
        self.assertTrue(all(f'draw.{name}' in diagnostics for name in PERF.DRAW_NAMES))
        # Synthetic quantiles test unit conversion independently of live fixture timings.
        diagnostics.update(parse.parse_state(
            ' diag draw.decay n=256 p50_ns=120000 p99_ns=250000'
            ' diag draw.fast n=1 p50_ns=0 p99_ns=pending'
            ' diag draw.render n=256 p50_ns=1500000 p99_ns=3000000'))
        self.assertEqual(diagnostics['draw.fast'][0]['p50_ns'], 0)
        self.assertEqual(diagnostics['draw.fast'][0]['p99_ns'], 'pending')
        row = {'point': 'A-5ns', 'proc_interval_s': 2.0,
               'before': {'proc': self.before},
               'after': {'proc': self.after, 'state': {'diagnostics': diagnostics}},
               'delta': parse.capture_delta(self.before, self.after)}
        report = PERF.markdown([row])
        self.assertIn('20.00 | 15.00 | 5.00', report)
        self.assertIn('0.120/0.250', report)
        self.assertIn('0.000/pending', report)
        self.assertIn('1.500/3.000', report)
        self.assertEqual(len({len(line.split('|')) for line in report.splitlines()}), 1)

    def test_state_preserves_duplicate_blocks_and_stops_at_trailing_tokens(self):
        diagnostics = parse.parse_state(
            'fps 0.0 diag draw.render n=1 p50_ns=7 trailing unrelated=99'
            ' diag draw.render n=2 p50_ns=9 restart ignored=42')
        self.assertEqual(diagnostics['draw.render'], [{'n': 1, 'p50_ns': 7}, {'n': 2, 'p50_ns': 9}])
        final = parse.parse_state(self.state + ' trailing unrelated=99')
        self.assertNotIn('unrelated', final['draw.view_render'][0])

    def test_real_state_fixture_is_accepted_by_transport(self):
        from unittest.mock import Mock
        sock, lease = Mock(), Mock()
        sock.makefile.return_value = io.BytesIO(self.state.encode())
        scpi = PERF.Scpi(sock, lease)
        try:
            self.assertEqual(scpi.state()['raw'], self.state.strip())
            sock.sendall.assert_called_once_with(b':SYSTem:STATe?\n')
        finally:
            scpi.close()

    def test_two_snapshots_one_socket_and_request_pacing(self):
        client, server = socket.socketpair()
        requests = []
        class Lease:
            def check(self):
                pass
        def serve():
            with server, server.makefile('rb') as reader:
                for _ in range(2):
                    requests.append((reader.readline(), time.monotonic()))
                    server.sendall(self.state.encode())
        worker = threading.Thread(target=serve)
        worker.start()
        scpi = PERF.Scpi(client, Lease())
        try:
            with patch.object(PERF, 'ssh', side_effect=[self.before_text, self.after_text]) as ssh:
                row = PERF.measure(scpi, 'example.invalid', 'A-1', 0.01)
            self.assertEqual(ssh.call_count, 2)
            self.assertGreaterEqual(row['state_interval_s'], 0.149)
            self.assertEqual(row['before']['state']['raw'], self.state.strip())
            self.assertEqual(row['delta']['gpu']['counters']['cpu_ticks'], 30)
        finally:
            scpi.close()
            client.close()
            worker.join(timeout=5)
        self.assertFalse(worker.is_alive())
        self.assertEqual([r[0] for r in requests], [b':SYSTem:STATe?\n'] * 2)
        self.assertGreaterEqual(requests[1][1] - requests[0][1], 0.149)

    def test_cli_writes_point_json_accumulates_report_and_preserves_existing_results(self):
        row = {'schema': 1, 'point': 'A-1', 'proc_interval_s': 2.0,
               'before': {'proc': self.before},
               'after': {'proc': self.after, 'state': {'diagnostics': parse.parse_state(self.state)}},
               'delta': parse.capture_delta(self.before, self.after)}
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory)
            previous = dict(row, point='previous')
            (out / 'previous.json').write_text(json.dumps(previous))
            arguments = ['perfmap.py', '--host', 'example.invalid', '--out', directory,
                         '--seconds', '1', '--settle', '0', 'A-1']
            with patch.object(sys, 'argv', arguments), patch.object(PERF.scope_lease, 'held') as held:
                with patch.object(PERF.socket, 'create_connection') as connect:
                    with patch.object(PERF, 'Scpi'), patch.object(PERF, 'measure', return_value=row):
                        with contextlib.redirect_stdout(io.StringIO()):
                            self.assertEqual(PERF.main(), 0)
            self.assertEqual(connect.call_count, 1)
            self.assertEqual(held.call_args.kwargs['cap'], 'observe')
            captured = (out / 'A-1.json').read_text()
            self.assertEqual(json.loads(captured)['point'], 'A-1')
            self.assertIn('previous', (out / 'report.md').read_text())
            self.assertIn('A-1', (out / 'report.md').read_text())
            with patch.object(sys, 'argv', arguments), patch.object(PERF.socket, 'create_connection') as connect:
                with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                    PERF.main()
                connect.assert_not_called()
            self.assertEqual((out / 'A-1.json').read_text(), captured)

    def test_capture_shell_syntax_is_valid_without_running_it(self):
        subprocess.run(['sh', '-n'], input=PERF.CAPTURE, text=True, check=True)

    def test_ssh_uses_existing_trust_port_and_identity_options(self):
        with patch.dict('os.environ', {'OOS_SSH_PORT': '2222', 'OOS_SSH_IDENTITY': '/path/to/key'}):
            with patch.object(PERF.subprocess, 'run') as run:
                run.return_value.stdout = 'captured'
                self.assertEqual(PERF.ssh('example.invalid', PERF.CAPTURE), 'captured')
        args = run.call_args.args[0]
        self.assertIn('StrictHostKeyChecking=yes', args)
        self.assertIn('BatchMode=yes', args)
        self.assertIn('2222', args)
        self.assertIn('/path/to/key', args)
        self.assertEqual(args[-2:], ['root@example.invalid', 'sh -s'])
        self.assertEqual(run.call_args.kwargs['input'], PERF.CAPTURE)
        self.assertTrue(run.call_args.kwargs['check'])

    def test_closed_or_invalid_scpi_and_transport_failure_stop(self):
        import io
        from unittest.mock import Mock
        invalid = [b'', b'fps incomplete', b'-113,"Unknown header"\n', b'fps 0.0 frames 1\n',
                   self.state.replace('fps ', 'records ', 1).encode(),
                   self.state.replace('diag draw.fast ', 'diag missing.fast ').encode(),
                   self.state.strip().encode() + b' diag draw.render n=1\n']
        for text in invalid:
            sock, lease = Mock(), Mock()
            sock.makefile.return_value = io.BytesIO(text)
            scpi = PERF.Scpi(sock, lease)
            with self.assertRaises(ValueError):
                scpi.state()
            scpi.close()
        for error, detail in [(subprocess.CalledProcessError(255, 'ssh', stderr='connection refused'),
                               'exited 255: connection refused'),
                              (subprocess.TimeoutExpired('ssh', 30, stderr=b'partial diagnostic'),
                               'timed out: partial diagnostic')]:
            with patch.object(PERF.subprocess, 'run', side_effect=error):
                with self.assertRaisesRegex(ValueError, detail):
                    PERF.ssh('example.invalid', PERF.CAPTURE)


if __name__ == '__main__':
    unittest.main()
