# SPDX-License-Identifier: GPL-2.0-or-later

"""PR metadata rules: branch, title, DCO, runner labels, size and link. Offline, no Git history needed."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


meta = load("oos_pr_meta", ROOT / "tools/ci/pr_meta.py")
MAIL = "someone@users." + "noreply.github.com"
SHA = "a" * 40


def commit(message, email=MAIL, name="Some One"):
    return meta.Commit(SHA, email, name, message)


class BranchTitleTests(unittest.TestCase):
    def test_branch_names(self):
        for ref in ("feat/123-scope-trigger", "fix/t0075-normal-trigger", "ci/t0092-pipeline", "docs/7-x"):
            with self.subTest(ref=ref):
                self.assertEqual(meta.check_branch(ref, "someone"), [])
        for ref in ("feature/123-x", "fix/t75-x", "fix/123", "fix/123-Upper", "overhaul", "fix/123-x/y",
                    "dependabot/cargo/syn-2"):
            with self.subTest(ref=ref):
                self.assertNotEqual(meta.check_branch(ref, "someone"), [])
        self.assertEqual(meta.check_branch("dependabot/cargo/syn-2", "dependabot[bot]"), [])

    def test_titles(self):
        for title in ("feat(ui): add a trigger menu", "fix: keep the cal backup", "refactor(hal)!: split the bus",
                      "build(deps): bump the actions group with 3 updates"):
            with self.subTest(title=title):
                self.assertEqual(meta.check_title(title), [])
        for title in ("Add a trigger menu", "feat:missing space", "feature(ui): x", "feat(UI): x", "fix: "):
            with self.subTest(title=title):
                self.assertNotEqual(meta.check_title(title), [])
        self.assertEqual(len(meta.check_title("fix: " + "x" * 68)), 1)
        self.assertEqual(meta.check_title("fix: " + "x" * 67), [])


class CommitTests(unittest.TestCase):
    def test_signoff_must_match_author(self):
        good = commit(f"fix: x\n\nWhy.\n\nSigned-off-by: Some One <{MAIL}>\n")
        self.assertEqual(meta.check_commits([good], "someone").errors, [])
        self.assertEqual(len(meta.check_commits([commit("fix: x\n")], "someone").errors), 1)
        other = commit("fix: x\n\nSigned-off-by: Other <other@users." + "noreply.github.com>\n")
        self.assertIn("does not match", meta.check_commits([other], "someone").errors[0])
        self.assertEqual(meta.check_commits([], "someone").errors, ["no commits between base and head"])

    def test_every_subject_is_a_finished_conventional_commit(self):
        sign = f"\n\nSigned-off-by: Some One <{MAIL}>\n"
        self.assertEqual(meta.check_commits([commit("fix(ui): keep the level" + sign)], "x").errors, [])
        for subject in ("fixup! fix(ui): keep the level", "squash! fix: x", "amend! fix: x", "WIP", "wip: more",
                        "[WIP] fix: x", "Keep the level", "fix: " + "x" * 68):
            with self.subTest(subject=subject):
                errors = meta.check_commits([commit(subject + sign)], "x").errors
                self.assertEqual(len(errors), 1, errors)
        self.assertIn("autosquash", meta.check_commits([commit("fixup! fix: x" + sign)], "x").errors[0])
        self.assertEqual(meta.check_commits([commit("fix: wipe the cache" + sign)], "x").errors, [])
        # Dependabot is exempt from the sign-off only: its subjects land on main too.
        bot = commit("Bump x from 1 to 2\n", email="bot@users." + "noreply.github.com", name="dependabot[bot]")
        self.assertEqual(len(meta.check_commits([bot], "dependabot[bot]").errors), 1)

    def test_dependabot_exempt_only_in_its_own_pr(self):
        bot = commit("build(deps): bump x\n", email="bot@users." + "noreply.github.com", name="dependabot[bot]")
        self.assertEqual(meta.check_commits([bot], "dependabot[bot]").errors, [])
        self.assertEqual(len(meta.check_commits([bot], "someone").errors), 1)

    def test_tool_details_only_warn(self):
        for trailer in ("Claude-Session: https://example.invalid/s", "Agent: tool/model",
                        "Co-authored-by: Claude <noreply@example.invalid>"):
            with self.subTest(trailer=trailer):
                r = meta.check_commits([commit(f"fix: x\n\n{trailer}\nSigned-off-by: Some One <{MAIL}>\n")], "x")
                self.assertEqual(r.errors, [])
                self.assertEqual(len(r.warnings), 1)
        human = commit(f"fix: x\n\nCo-authored-by: Pat <pat@example.invalid>\nSigned-off-by: Some One <{MAIL}>\n")
        self.assertEqual(meta.check_commits([human], "x").warnings, [])


class RunnerTests(unittest.TestCase):
    def test_only_literal_hosted_labels(self):
        ok = ("jobs:\n  a:\n    runs-on: ubuntu-24.04\n  b:\n    runs-on: 'ubuntu-24.04-arm'  # native\n"
              "  c:\n    runs-on: ubuntu-latest\n")
        self.assertEqual(meta.check_runners({"w.yml": ok}), [])
        for bad in ("runs-on: self-hosted", "runs-on: [self-hosted, linux]", "runs-on: ${{ matrix.os }}",
                    "runs-on: proxmox", "  - runs-on: macos-15", "runs-on: ubuntu-22.04", "runs-on: windows-latest"):
            with self.subTest(bad=bad):
                self.assertEqual(len(meta.check_runners({"w.yml": f"jobs:\n  a:\n    {bad}\n"})), 1)

    def test_larger_runners_are_rejected(self):
        for label in ("ubuntu-24.04-16core", "ubuntu-latest-4-cores", "ubuntu-24.04-arm-8core", "ubuntu-24.04-gpu",
                      "macos-latest-xlarge", "macos-15-large", "linux-arm64-64core"):
            with self.subTest(label=label):
                errors = meta.check_runners({"w.yml": f"jobs:\n  a:\n    runs-on: {label}\n"})
                self.assertEqual(len(errors), 1)
                self.assertIn("no larger or self-hosted runners", errors[0])
        group = "jobs:\n  a:\n    runs-on:\n      group: larger\n      labels: ubuntu-24.04-16core\n"
        self.assertEqual(len(meta.check_runners({"w.yml": group})), 1)

    def test_repository_workflows_pass(self):
        self.assertEqual(meta.check_runners(meta.workflows(ROOT)), [])


class SizeLinkAgentTests(unittest.TestCase):
    def test_size_excludes_generated_data(self):
        numstat = ("300\t50\tui/src/a.rs\n"
                   "900\t0\tui/tests/golden/demo/normal/x.ppm\n"
                   "40\t0\thal/tests/fixtures/f.bin\n"
                   "500\t500\tCargo.lock\n"
                   "10\t10\txtask/data/ui/clippy.txt\n"
                   "-\t-\tbinary.png\n"
                   "60\t0\tdocs/golden.md\n")
        self.assertEqual(meta.changed_lines(numstat), 410)
        self.assertEqual(len(meta.check_size(410)), 1)
        self.assertEqual(meta.check_size(400), [])

    def test_link(self):
        for body in ("Fixes #12", "Task: docs/tasks/0092-pipeline.md", "(#3)"):
            self.assertEqual(meta.check_link(body), [], body)
        for body in ("", "see issue 12", "abc#12"):
            self.assertEqual(len(meta.check_link(body)), 1, body)

    def test_agent_label_matches_author(self):
        self.assertEqual(meta.check_agent(meta.AGENT_APP, ["agent"]), [])
        self.assertEqual(meta.check_agent("someone", []), [])
        self.assertEqual(len(meta.check_agent(meta.AGENT_APP, [])), 1)
        self.assertEqual(len(meta.check_agent("someone", ["agent"])), 1)


if __name__ == "__main__":
    unittest.main()
