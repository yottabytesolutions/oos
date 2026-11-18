# SPDX-License-Identifier: GPL-2.0-or-later

"""A stacked pull request (CONTRIBUTING.md#stacked-pull-requests) targets the branch of the layer below it, not main. pr_meta, changes.py and the
Codex gate judge only the layer's own commits. That the required workflows run on such a pull request is checked in
xtask (xtask/src/stack_ci.rs), with a YAML parser. Offline: a throwaway Git repository, no network."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
MAIL = "someone@users." + "noreply.github.com"
LOWER, UPPER = "docs/11-stacked-prs-process", "ci/11-stack-aware-checks"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


meta = load("oos_pr_meta_stack", ROOT / "tools/ci/pr_meta.py")
gate = load("oos_codex_gate_stack", ROOT / "tools/ci/codex_gate.py")
changes = load("oos_ci_changes_stack", ROOT / "tools/ci/changes.py")


def run(cwd, *args):
    return subprocess.run(["git", "-c", "user.name=Some One", "-c", f"user.email={MAIL}", "-c",
                           "commit.gpgsign=false", *args], cwd=cwd, check=True, capture_output=True, text=True,
                          timeout=60).stdout.strip()


class StackRepo:
    """main, a lower layer with an unsigned commit, an upper layer with a signed one."""

    def __init__(self, root):
        self.root = root
        run(root, "init", "-q", "-b", "main")
        self.main = self.commit("README.md", "x\n", "docs: start", signed=True)
        run(root, "checkout", "-q", "-b", LOWER)
        self.lower = self.commit("docs/a.md", "a\n" * 500, "docs: the lower layer", signed=False)
        run(root, "checkout", "-q", "-b", UPPER)
        self.upper = self.commit("tools/ci/b.py", "b\n" * 10, "ci: the upper layer", signed=True)

    def commit(self, rel, text, subject, signed):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
        run(self.root, "add", rel)
        run(self.root, "commit", "-q", *(["-s"] if signed else []), "-m", subject)
        return run(self.root, "rev-parse", "HEAD")


class StackedLayerTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.repo = StackRepo(Path(self.tmp.name))

    def pr_meta(self, base, head, ref=UPPER):
        env = {"PR_HEAD_REF": ref, "PR_TITLE": "ci: run every check on stacked pull requests",
               "PR_BODY": "Closes #11", "PR_BASE_SHA": base, "PR_HEAD_SHA": head, "PR_AUTHOR": "someone",
               "PR_LABELS": "[]"}
        out = []
        cwd = os.getcwd()
        os.chdir(self.repo.root)
        try:
            with mock.patch.dict(os.environ, env), mock.patch("builtins.print", side_effect=out.append):
                code = meta.main(["pr"])
        finally:
            os.chdir(cwd)
        return code, out

    def test_layer_names_follow_the_branch_scheme(self):
        for ref in (LOWER, UPPER):
            with self.subTest(ref=ref):
                self.assertEqual(meta.check_branch(ref, "someone"), [])

    def test_pr_meta_judges_only_the_commits_of_its_own_layer(self):
        code, out = self.pr_meta(self.repo.lower, self.repo.upper)
        self.assertEqual(code, 0, out)
        self.assertEqual(out[-1], "pr-meta: 0 error(s), 0 warning(s)")

    def test_pr_meta_still_sees_the_lower_layer_against_main(self):
        code, out = self.pr_meta(self.repo.main, self.repo.upper)
        self.assertEqual(code, 1)
        self.assertTrue(any("no 'Signed-off-by:'" in line for line in out), out)
        self.assertTrue(any("changed lines" in line for line in out), out)

    def test_changes_diffs_against_the_lower_layer(self):
        env = {"EVENT": "pull_request", "BASE": self.repo.lower}
        with mock.patch.object(changes, "git", lambda *a: run(self.repo.root, *a)):
            self.assertFalse(changes.decide("rust", "clippy", env).run)
            self.assertTrue(changes.decide("all", "fast", env).run)

    def test_codex_gate_does_not_depend_on_the_base_branch(self):
        head = self.repo.upper
        state = {"code_review": {"commit_sha": head, "status": "completed", "has_unresolved_findings": False}}
        body = f"<!-- codex-review-state:v1 {json.dumps(state)} -->"
        comments = [{"user": {"login": gate.BOT}, "updated_at": "2026-11-17T08:00:00Z", "body": body}]
        d = Path(self.tmp.name) / "gate"
        d.mkdir()
        (d / "pr.json").write_text(json.dumps({"head": {"sha": head, "ref": UPPER}, "base": {"ref": LOWER}}))
        (d / "issue_comments.json").write_text(json.dumps(comments))
        (d / "reviews.json").write_text("[]")
        with mock.patch("builtins.print"):
            self.assertEqual(gate.main(["offline", "--dir", str(d)]), 0)


if __name__ == "__main__":
    unittest.main()
