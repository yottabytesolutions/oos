# SPDX-License-Identifier: GPL-2.0-or-later
"""The host budget contract fails closed on missing and regressed observations."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location('perf_budgets', ROOT / 'tools/perf-budgets.py')
BUDGET = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(BUDGET)


class PerfBudgets(unittest.TestCase):
    def setUp(self):
        self.rows = BUDGET.budgets((ROOT / 'docs/architecture/README.md').read_text())
        self.lines = [f'PERF_BUDGET {name} 1000 100 10000 1000' for name in self.rows]
        self.lines += [f'ALLOC_BUDGET {name} 64 0' for name in BUDGET.ALLOCATIONS]

    def test_real_table_and_samplers(self):
        self.assertEqual(len(self.rows), 13)
        BUDGET.sampler_copies_match(ROOT)
        self.assertIn('13 timing budgets', BUDGET.check(self.rows, '\n'.join(self.lines)))

    def test_missing_unknown_and_nonpositive_fail(self):
        for lines in [[], self.lines[1:], self.lines + ['PERF_BUDGET unknown 1 1 1 1'],
                      self.lines + [f'PERF_BUDGET {next(iter(self.rows))} 0 1 1 1']]:
            with self.assertRaises(ValueError):
                BUDGET.check(self.rows, '\n'.join(lines))

    def test_best_or_tail_regression_cannot_hide_in_other_cases(self):
        name = next(iter(self.rows))
        for values in ['100 1000 10000 1000', '1000 100 1000 10000']:
            with self.assertRaises(ValueError):
                BUDGET.check(self.rows, '\n'.join(self.lines + [f'PERF_BUDGET {name} {values}']))

    def test_full_batch_learning_timing_is_required(self):
        name = 'perf_guard_batch_core_learning'
        self.assertIn(name, self.rows)
        lines = [line for line in self.lines if not line.startswith(f'PERF_BUDGET {name} ')]
        with self.assertRaises(ValueError):
            BUDGET.check(self.rows, '\n'.join(lines))

    def test_pipeline_counter_staging_overhead_is_required_and_capped_at_two_percent(self):
        name = 'perf_guard_pipeline_counters'
        self.assertEqual(self.rows[name], (1.02, 1.515))
        missing = [line for line in self.lines if not line.startswith(f'PERF_BUDGET {name} ')]
        with self.assertRaises(ValueError):
            BUDGET.check(self.rows, '\n'.join(missing))
        for candidate in [1021, 1030]:
            with self.assertRaises(ValueError):
                BUDGET.check(self.rows, '\n'.join(self.lines + [f'PERF_BUDGET {name} 1000 {candidate} 1000 1000']))
        for candidate in [1010, 1017, 1020]:
            BUDGET.check(self.rows, '\n'.join(self.lines + [f'PERF_BUDGET {name} 1000 {candidate} 1000 1515']))
        with self.assertRaises(ValueError):
            BUDGET.check(self.rows, '\n'.join(self.lines + [f'PERF_BUDGET {name} 1000 1020 1000 1516']))

    def test_pipeline_staging_limit_matches_executable_and_keeps_work_checks(self):
        source = (ROOT / 'ui/src/ui/draw_routes/counters.rs').read_text()
        self.assertIn('assert!(ratio <= 1.02', source)
        self.assertIn('pair.check_tail(1.515)', source)
        for contract in ['for channels in [1, 2, 4]', 'for tdiv in [1e-6, 10e-6]',
                         'for _ in 0..8', 'measure(crate::perfguard::ROUNDS, 16, baseline, counted)',
                         'assert_eq!(build.route.ui.batch_picture.frames as usize, build.frames)',
                         'build.route.ui.wfm_total - before',
                         '(2 * crate::perfguard::ROUNDS * 16 * build.frames) as u64',
                         'sampled claims must be exercised', 'compare(&picture(',
                         'reference.route.ui.fast_take, candidate.route.ui.fast_take',
                         'reference.route.ui.wfm_total, candidate.route.ui.wfm_total']:
            self.assertIn(contract, source)

    def test_each_pipeline_case_still_rejects_best_or_cpu_tail_regression(self):
        name = 'perf_guard_pipeline_counters'
        good = f'PERF_BUDGET {name} 1000 1020 1000 1515'
        for case in range(6):
            for values in ['1000 1021 1000 1515', '1000 1020 1000 1516']:
                cases = [good] * 6
                cases[case] = f'PERF_BUDGET {name} {values}'
                with self.subTest(case=case, values=values), self.assertRaises(ValueError):
                    BUDGET.check(self.rows, '\n'.join(self.lines + cases))

    def test_freed_allocations_and_missing_paths_fail(self):
        for lines in [self.lines[:-1], self.lines + ['ALLOC_BUDGET deposit_spans 64 1'],
                      self.lines + ['ALLOC_BUDGET deposit_dots 0 0']]:
            with self.assertRaises(ValueError):
                BUDGET.check(self.rows, '\n'.join(lines))

    def test_both_core_matching_probes_are_required(self):
        for name in ['core_matching_learn_and_decode', 'core_matching_worker_decode']:
            with self.subTest(probe=name):
                self.assertIn(name, BUDGET.ALLOCATIONS)
                lines = [line for line in self.lines if not line.startswith(f'ALLOC_BUDGET {name} ')]
                with self.assertRaises(ValueError):
                    BUDGET.check(self.rows, '\n'.join(lines))

    def test_malformed_and_duplicate_tables_fail(self):
        for text in ['', '### Host budget rows\n', '### Host budget rows\n| x | nan | 2 | a | b |',
                     '### Host budget rows\n| x | 1 | 2 | a | b |\n| x | 1 | 2 | a | b |']:
            with self.assertRaises((ValueError, IndexError)):
                BUDGET.budgets(text)

    def test_divergent_samplers_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for path in ['ui/src/system/perfguard.rs', 'hal/tests/perf_support/samples.rs']:
                (root / path).parent.mkdir(parents=True)
                (root / path).write_text(path)
            with self.assertRaises(ValueError):
                BUDGET.sampler_copies_match(root)
