# SPDX-License-Identifier: GPL-2.0-or-later

"""Release notes section for production code and the `Code growth:` rule (ADR 0035)."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools/ci/code_growth.py"
spec = importlib.util.spec_from_file_location("oos_code_growth", SCRIPT)
growth = importlib.util.module_from_spec(spec)
spec.loader.exec_module(growth)

CHANGELOG = """# Changes

## 0.2.1: Smaller

Code growth: the new decoder
needs its tables.

## 0.2.10

Code growth: not this one.

## 0.2.1-rc1

Code growth: nor the release candidate.

## 0.2.0: Hard fork

Text.
"""


def loc(delta):
    return {"base": "v0.2.0", "production": {}, "production_total": {"now": 10, "base": 10 - delta, "delta": delta}}


class SectionTests(unittest.TestCase):
    def test_a_section_ends_at_the_next_heading_and_a_longer_version_is_another(self):
        self.assertIn("new decoder", growth.section(CHANGELOG, "0.2.1"))
        self.assertNotIn("not this one", growth.section(CHANGELOG, "0.2.1"))
        self.assertNotIn("release candidate", growth.section(CHANGELOG, "0.2.1"))
        self.assertIn("release candidate", growth.section(CHANGELOG, "0.2.1-rc1"))
        self.assertEqual(growth.section(CHANGELOG, "0.2.0").strip(), "Text.")
        self.assertEqual(growth.section(CHANGELOG, "0.3.0"), "")

    def test_the_reason_is_a_paragraph_that_starts_with_the_mark(self):
        why = growth.reason("Release 0.2.1\n\nmention of Code growth: inline", growth.section(CHANGELOG, "0.2.1"))
        self.assertEqual(why, "Code growth: the new decoder needs its tables.")
        self.assertIsNone(growth.reason("Code growth? no", ""))
        self.assertIsNone(growth.reason("Code growth:  \n\nother", "  Code growth:\n"))


class NotesTests(unittest.TestCase):
    def test_growth_needs_a_reason_and_shrinking_does_not(self):
        self.assertFalse(growth.notes(loc(5), "| t |", None)[1])
        text, ok = growth.notes(loc(5), "| t |", "Code growth: why.")
        self.assertTrue(ok)
        self.assertEqual(text, "## Production code\n\nCompared with v0.2.0 (ADR 0035).\n\n| t |\n\nCode growth: why.\n")
        self.assertTrue(growth.notes(loc(-3), "| t |", None)[1])
        self.assertTrue(growth.notes(loc(0), "| t |", None)[1])

    def test_the_script_fails_with_a_message_on_unexplained_growth(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d)
            (p / "loc.json").write_text(json.dumps(loc(7)), encoding="utf-8")
            (p / "loc.md").write_text("| t |\n", encoding="utf-8")
            (p / "tag.txt").write_text("Release 0.3.0\n", encoding="utf-8")
            (p / "CHANGELOG.md").write_text(CHANGELOG, encoding="utf-8")
            args = [sys.executable, "-I", str(SCRIPT), "--loc", str(p / "loc.json"), "--table", str(p / "loc.md"),
                    "--tag-message", str(p / "tag.txt"), "--changelog", str(p / "CHANGELOG.md")]
            failed = subprocess.run([*args, "--version", "0.3.0"], capture_output=True, text=True, check=False)
            self.assertEqual(failed.returncode, 1)
            self.assertIn("production code grew by 7 lines since v0.2.0", failed.stderr)
            passed = subprocess.run([*args, "--version", "0.2.1"], capture_output=True, text=True, check=False)
            self.assertEqual(passed.returncode, 0, passed.stderr)
            self.assertIn("Code growth: the new decoder needs its tables.", passed.stdout)


if __name__ == "__main__":
    unittest.main()
