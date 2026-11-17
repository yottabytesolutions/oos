# SPDX-License-Identifier: GPL-2.0-or-later

"""The path filter of the PR jobs (tools/ci/changes.py). Offline: Git and the API are replaced."""
import importlib.util
from pathlib import Path
import re
import subprocess
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


changes = load("oos_ci_changes", ROOT / "tools/ci/changes.py")
HEAD = "b" * 40
TREE = "c" * 40
REF = f"refs/heads/gh-readonly-queue/main/pr-42-{'a' * 40}"


def check_runs(name, conclusion="success", slug="github-actions"):
    return {"check_runs": [{"name": name, "conclusion": conclusion, "app": {"slug": slug}}]}


class PathTests(unittest.TestCase):
    def test_documents_settings_and_ci_scripts_skip_rust(self):
        for path in ("docs/development.md", "README.md", "ui/README.md", "AGENTS.md", ".github/labeler.yml",
                     ".github/workflows/stale.yml", ".github/rulesets/main.json", "tools/ci/pr_meta.py"):
            with self.subTest(path=path):
                self.assertFalse(changes.needs_rust(path))

    def test_code_manifests_and_own_inputs_need_rust(self):
        for path in ("ui/src/ui.rs", "Cargo.lock", "deny.toml", "rust-toolchain.toml", ".cargo/config.toml",
                     "rootfs/initramfs/bin/oos-watchdog", "tools/privacy/scan.py", "xtask/data/size.txt",
                     ".github/workflows/pr.yml", ".github/workflows/image.yml", "tools/ci/changes.py"):
            with self.subTest(path=path):
                self.assertTrue(changes.needs_rust(path))

    def test_every_file_a_rust_source_includes_needs_rust(self):
        """include_str!/include_bytes! of a skipped path would let a change skip the test that reads it."""
        sources = subprocess.run(["git", "ls-files", "*.rs"], cwd=ROOT, check=True, capture_output=True,
                                 text=True).stdout.split()
        include = re.compile(r'include_(?:str|bytes)!\(\s*"([^"]+)"')
        found = 0
        for source in sources:
            text = (ROOT / source).read_text(encoding="utf-8", errors="replace")
            for target in include.findall(text):
                path = (ROOT / source).parent.joinpath(target).resolve()
                if not path.is_relative_to(ROOT):
                    continue
                found += 1
                rel = path.relative_to(ROOT).as_posix()
                with self.subTest(source=source, path=rel):
                    self.assertTrue(changes.needs_rust(rel))
        self.assertGreater(found, 0)

    def test_decision_by_paths(self):
        self.assertFalse(changes.by_paths(["docs/a.md", ".github/CODEOWNERS"]).run)
        self.assertTrue(changes.by_paths(["docs/a.md", "core/src/lib.rs"]).run)
        self.assertTrue(changes.by_paths([]).run)


class DecideTests(unittest.TestCase):
    def decide(self, scope, env, diff="docs/a.md\n", api=None, tree=TREE):
        def git(*args):
            if args[0] == "diff":
                if isinstance(diff, Exception):
                    raise diff
                return diff
            return tree

        def get(path, token):
            if api is None:
                raise RuntimeError("no API")
            for key, value in api.items():
                if key in path:
                    return value
            raise RuntimeError(f"unexpected {path}")

        with mock.patch.object(changes, "git", git), mock.patch.object(changes, "get", get):
            return changes.decide(scope, "clippy", env)

    def test_push_and_unknown_events_run(self):
        self.assertTrue(self.decide("rust", {"EVENT": "push", "BASE": "x"}).run)
        self.assertTrue(self.decide("rust", {}).run)

    def test_pull_request_filters_by_path(self):
        pr = {"EVENT": "pull_request", "BASE": "a" * 40}
        self.assertFalse(self.decide("rust", pr).run)
        self.assertTrue(self.decide("rust", pr, diff="docs/a.md\nhal/src/lib.rs\n").run)
        self.assertTrue(self.decide("all", pr).run)

    def test_missing_base_or_failed_diff_runs(self):
        self.assertTrue(self.decide("rust", {"EVENT": "pull_request", "BASE": ""}).run)
        failed = subprocess.CalledProcessError(128, ["git"])
        self.assertTrue(self.decide("rust", {"EVENT": "pull_request", "BASE": "x"}, diff=failed).run)

    def queue(self, scope="all", runs=None, tree=TREE, ref=REF):
        api = {"/pulls/42": {"head": {"sha": HEAD}}, f"/git/commits/{HEAD}": {"tree": {"sha": TREE}},
               "/check-runs": runs if runs is not None else check_runs("clippy")}
        env = {"EVENT": "merge_group", "BASE": "a" * 40, "QUEUE_REF": ref, "REPO": "o/r", "GITHUB_TOKEN": "t"}
        return self.decide(scope, env, diff="core/src/lib.rs\n", api=api, tree=tree)

    def test_queue_skips_a_tree_that_passed_on_the_pr_head(self):
        d = self.queue()
        self.assertFalse(d.run)
        self.assertIn("#42", d.reason)

    def test_queue_runs_when_the_tree_or_the_result_differs(self):
        self.assertTrue(self.queue(tree="d" * 40).run)
        self.assertTrue(self.queue(runs=check_runs("clippy", "failure")).run)
        self.assertTrue(self.queue(runs=check_runs("clippy", slug="other-app")).run)
        self.assertTrue(self.queue(runs=check_runs("fast")).run)
        self.assertTrue(self.queue(runs={"check_runs": []}).run)
        self.assertTrue(self.queue(ref="refs/heads/main").run)

    def test_queue_runs_without_the_api(self):
        env = {"EVENT": "merge_group", "BASE": "a" * 40, "QUEUE_REF": REF, "REPO": "o/r", "GITHUB_TOKEN": "t"}
        self.assertTrue(self.decide("all", env).run)

    def test_queue_ref_parsing(self):
        self.assertEqual(changes.queue_pr(REF), 42)
        self.assertEqual(changes.queue_pr(f"refs/heads/gh-readonly-queue/release/x/pr-7-{'f' * 40}"), 7)
        self.assertIsNone(changes.queue_pr("refs/heads/gh-readonly-queue/main/pr-x-1"))


class WorkflowTests(unittest.TestCase):
    def test_cache_warm_up_builds_the_crates_the_gate_tests(self):
        crates = re.findall(r'Crate \{ dir: "([a-z]+)"', (ROOT / "xtask/src/repo.rs").read_text())
        loop = re.search(r"for crate in ([a-z ]+); do", (ROOT / ".github/workflows/pr.yml").read_text())
        self.assertTrue(crates)
        self.assertIsNotNone(loop)
        self.assertEqual(loop.group(1).split(), crates)


class OutputTests(unittest.TestCase):
    def test_writes_the_github_output(self):
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "out"
            env = {"EVENT": "push", "GITHUB_OUTPUT": str(out)}
            with mock.patch.dict("os.environ", env, clear=True), mock.patch("builtins.print"):
                self.assertEqual(changes.main(["--scope", "rust", "--check", "clippy"]), 0)
            self.assertEqual(out.read_text(), "run=true\n")


if __name__ == "__main__":
    unittest.main()
