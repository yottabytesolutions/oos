# SPDX-License-Identifier: GPL-2.0-or-later

"""Job and step durations for the PR job summary (tools/ci/run_summary.py). Offline."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


summary = load("oos_ci_run_summary", ROOT / "tools/ci/run_summary.py")


def step(name, start, end, conclusion="success"):
    return {"name": name, "started_at": start, "completed_at": end, "conclusion": conclusion}


def job(name, steps, status="completed", created="2026-11-17T10:00:00Z", started="2026-11-17T10:00:05Z",
        completed="2026-11-17T10:03:10Z"):
    return {"name": name, "status": status, "conclusion": "success", "created_at": created, "started_at": started,
            "completed_at": completed, "steps": steps}


class SummaryTests(unittest.TestCase):
    def test_durations_and_order(self):
        jobs = summary.parse([
            job("clippy", [step("Set up job", "2026-11-17T10:00:05Z", "2026-11-17T10:00:07Z"),
                           step("Clippy", "2026-11-17T10:00:07Z", "2026-11-17T10:03:00Z"),
                           step("Skipped", "2026-11-17T10:03:00Z", "2026-11-17T10:03:00Z", "skipped")]),
            job("fast", [], completed="2026-11-17T10:00:50Z"),
            job("timings", [], status="in_progress", completed=None),
            job("deny", [], started="0001-01-01T00:00:00Z"),
        ], exclude="timings")
        self.assertEqual([j.name for j in jobs], ["clippy", "fast"])
        self.assertEqual((jobs[0].queued, jobs[0].seconds), (5, 185))
        self.assertEqual([(s.name, s.seconds) for s in jobs[0].steps], [("Set up job", 2), ("Clippy", 173)])
        text = summary.markdown(jobs)
        self.assertLess(text.index("| clippy | success | 0:05 | 3:05 |"), text.index("| fast |"))
        self.assertIn("| Clippy | 2:53 |", text)
        self.assertNotIn("Skipped", text)
        self.assertNotIn("fast: steps", text)

    def test_excluded_job_and_empty_run(self):
        self.assertEqual(summary.parse([job("timings", [])], exclude="timings"), [])
        self.assertEqual(summary.markdown([]), "No completed jobs.\n")


if __name__ == "__main__":
    unittest.main()
