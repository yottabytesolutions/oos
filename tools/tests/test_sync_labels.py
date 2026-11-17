# SPDX-License-Identifier: GPL-2.0-or-later

"""The label file parses, and every label the pipeline uses is defined."""
import importlib.util
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


labels = load("oos_sync_labels", ROOT / "tools/ci/sync_labels.py")


class LabelTests(unittest.TestCase):
    def test_repository_labels_parse(self):
        names = {x.name for x in labels.parse((ROOT / ".github/labels.yml").read_text())}
        for required in ("risk/hardware", "needs-unit", "agent-ready", "good-first-issue", "flaky", "agent", "stale"):
            self.assertIn(required, names)

    def test_every_used_label_is_defined(self):
        names = {x.name for x in labels.parse((ROOT / ".github/labels.yml").read_text())}
        used = set(re.findall(r"^([\w/-]+):\n", (ROOT / ".github/labeler.yml").read_text(), re.MULTILINE))
        for path in [*(ROOT / ".github/ISSUE_TEMPLATE").glob("*.yml"), ROOT / ".github/dependabot.yml"]:
            for group in re.findall(r"labels: \[([^\]]*)\]", path.read_text()):
                used |= {x.strip() for x in group.split(",") if x.strip()}
        used |= set(re.findall(r"--(?:add-)?label ([\w/-]+)", "".join(
            p.read_text() for p in (ROOT / ".github/workflows").glob("*.yml"))))
        used |= set(re.findall(r"(?:stale-pr-label|exempt-pr-labels): ([\w/-]+)",
                               (ROOT / ".github/workflows/stale.yml").read_text()))
        self.assertTrue(used)
        self.assertEqual(used - names, set())

    def test_malformed_entries_fail(self):
        for text in ("- name: a\n  color: zzzzzz\n  description: d\n", "- name: a\n  color: 000000\n",
                     "  name: a\n", "- name: a\n  color: 000000\n  description: d\n- name: a\n  color: 000000\n"
                     "  description: d\n", "name: a\n"):
            with self.subTest(text=text), self.assertRaises(ValueError):
                labels.parse(text)


if __name__ == "__main__":
    unittest.main()
