# SPDX-License-Identifier: GPL-2.0-or-later

"""The codex-review gate decides from the Codex summary marker only. Offline: no network."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


gate = load("oos_codex_gate", ROOT / "tools/ci/codex_gate.py")

HEAD = "5e683947b9547f325f4e6e1c99a0b1bd9e6c647c"
OLD = "f617d629536ef616c0c5d13ec3774c06cb7b7662"


def marker(sha=HEAD, status="completed", unresolved=False):
    state = {"code_review": {"commit_sha": sha, "has_any_findings": unresolved,
                             "has_unresolved_findings": unresolved, "status": status},
             "security_review": {"commit_sha": None, "has_any_findings": None,
                                 "has_unresolved_findings": None, "status": None}}
    return ("<!-- codex-pull-request-review-summary -->\n"
            f"<!-- codex-review-state:v1 {json.dumps(state, separators=(',', ':'))} -->\n\n<details>summary</details>")


def bot(body, when="2026-11-17T07:35:22Z"):
    return gate.Comment(gate.BOT, when, body)


class DecideTests(unittest.TestCase):
    def test_clean_review_of_the_head_passes(self):
        verdict = gate.decide(HEAD, [bot(marker())])
        self.assertTrue(verdict.passed)

    def test_no_marker_is_pending_not_passing(self):
        for comments in ([], [bot("plain text")], [gate.Comment("someone", "2026-11-17T09:00:00Z", marker())]):
            with self.subTest(comments=comments):
                verdict = gate.decide(HEAD, comments)
                self.assertEqual(verdict.state, gate.State.PENDING)

    def test_review_of_another_commit_does_not_pass(self):
        verdict = gate.decide(HEAD, [bot(marker(sha=OLD))])
        self.assertEqual(verdict.state, gate.State.PENDING)
        self.assertIn("f617d62953", verdict.message)

    def test_running_review_does_not_pass(self):
        self.assertEqual(gate.decide(HEAD, [bot(marker(status="in_progress"))]).state, gate.State.PENDING)
        self.assertEqual(gate.decide(HEAD, [bot(marker(status=None))]).state, gate.State.PENDING)

    def test_unresolved_or_unknown_findings_fail(self):
        for unresolved in (True, None):
            with self.subTest(unresolved=unresolved):
                verdict = gate.decide(HEAD, [bot(marker(unresolved=unresolved))])
                self.assertEqual(verdict.state, gate.State.FAILURE)

    def test_unparseable_marker_fails_closed(self):
        for body in ("<!-- codex-review-state:v1 {not json} -->", "<!-- codex-review-state:v1 {\"x\":1} -->",
                     "<!-- codex-review-state:v1 {\"code_review\":{\"has_unresolved_findings\":\"no\"}} -->"):
            with self.subTest(body=body):
                self.assertEqual(gate.decide(HEAD, [bot(body)]).state, gate.State.FAILURE)

    def test_newest_bot_marker_decides(self):
        stale = bot(marker(unresolved=True), "2026-11-17T07:00:00Z")
        fresh = bot(marker(), "2026-11-17T08:00:00Z")
        self.assertTrue(gate.decide(HEAD, [fresh, stale]).passed)
        self.assertFalse(gate.decide(HEAD, [bot(marker(), "2026-11-17T07:00:00Z"),
                                            bot(marker(unresolved=True), "2026-11-17T08:00:00Z")]).passed)

    def test_impostor_marker_is_ignored(self):
        impostor = gate.Comment("chatgpt-codex-connector", "2026-11-17T09:00:00Z", marker())
        self.assertFalse(gate.decide(HEAD, [bot(marker(unresolved=True)), impostor]).passed)


class ApiShapeTests(unittest.TestCase):
    def test_issue_comments_use_updated_at_and_reviews_submitted_at(self):
        issue = [{"user": {"login": gate.BOT}, "updated_at": "2026-11-17T08:00:00Z", "body": marker()}]
        reviews = [{"user": {"login": gate.BOT}, "submitted_at": "2026-11-17T07:00:00Z",
                    "body": marker(unresolved=True)}, {"user": None, "submitted_at": "x", "body": "y"}]
        comments = gate.comments_from(issue, reviews)
        self.assertEqual(len(comments), 2)
        self.assertTrue(gate.decide(HEAD, comments).passed)

    def test_offline_mode_reads_rest_shapes(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d)
            (root / "pr.json").write_text(json.dumps({"head": {"sha": HEAD}}))
            (root / "issue_comments.json").write_text(json.dumps(
                [{"user": {"login": gate.BOT}, "updated_at": "2026-11-17T08:00:00Z", "body": marker(sha=OLD)}]))
            (root / "reviews.json").write_text("[]")
            with mock.patch("builtins.print"):
                self.assertEqual(gate.main(["offline", "--dir", d]), 1)
                (root / "issue_comments.json").write_text(json.dumps(
                    [{"user": {"login": gate.BOT}, "updated_at": "2026-11-17T08:00:00Z", "body": marker()}]))
                self.assertEqual(gate.main(["offline", "--dir", d]), 0)
                (root / "pr.json").write_text("{}")
                self.assertEqual(gate.main(["offline", "--dir", d]), 2)

    def test_pending_review_fails_the_job_only_without_a_published_status(self):
        def request(method, path, payload=None):
            return {"head": {"sha": HEAD}} if method == "GET" else {}

        with mock.patch("builtins.print"), mock.patch.object(gate.Api, "request", side_effect=request), \
                mock.patch.object(gate.Api, "pages", return_value=[]):
            self.assertEqual(gate.main(["pr", "--repo", "a/b", "--pr", "6", "--publish"]), 0)
            self.assertEqual(gate.main(["pr", "--repo", "a/b", "--pr", "6"]), 1)
        unresolved = [{"user": {"login": gate.BOT}, "updated_at": "2026-11-17T08:00:00Z",
                       "body": marker(unresolved=True)}]
        with mock.patch("builtins.print"), mock.patch.object(gate.Api, "request", side_effect=request), \
                mock.patch.object(gate.Api, "pages", side_effect=[unresolved, []]):
            self.assertEqual(gate.main(["pr", "--repo", "a/b", "--pr", "6", "--publish"]), 1)

    def test_queue_refuses_bad_input_without_network(self):
        with mock.patch("builtins.print"), mock.patch.object(gate.Api, "request") as request:
            self.assertEqual(gate.main(["queue", "--repo", "a/b", "--sha", "HEAD"]), 2)
            self.assertEqual(gate.main(["queue", "--repo", "a b", "--sha", HEAD]), 2)
            self.assertEqual(gate.main(["queue", "--repo", "a/b", "--sha", HEAD, "--publish"]), 0)
            request.assert_called_once()
            self.assertEqual(request.call_args.args[2]["context"], "codex-review")


if __name__ == "__main__":
    unittest.main()
