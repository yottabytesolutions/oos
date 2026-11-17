#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""The `codex-review` gate: a PR passes only when Codex reviewed its head commit and left nothing unresolved.

Codex (the chatgpt-codex-connector app) keeps one summary comment per PR and edits it in place. The comment carries
`<!-- codex-review-state:v1 {json} -->`. The newest marker from the bot decides. Anything else fails closed: no
marker, a marker that does not parse, a review of another commit, a review still running, unresolved findings.

  codex_gate.py pr --repo OWNER/NAME --pr N [--publish] [--target-url URL]
  codex_gate.py offline --dir DIR          pr.json, issue_comments.json, reviews.json as the REST API returns them
  codex_gate.py queue --repo OWNER/NAME --sha SHA [--publish] [--target-url URL]

`--publish` sets the commit status `codex-review` on the head commit (needs `statuses: write`). Exit status: 0 pass
(with `--publish` also pending: the status carries it), 1 not passing, 2 input or API error.
"""
import argparse
from dataclasses import dataclass
from enum import Enum
import json
import os
from pathlib import Path
import re
import sys
import time
from typing import Final
import urllib.error
import urllib.request

BOT: Final = "chatgpt-codex-connector[bot]"
CONTEXT: Final = "codex-review"
MARKER: Final = re.compile(r"<!--\s*codex-review-state:v1\s+(\{.*?\})\s*-->", re.DOTALL)
API: Final = "https://api.github.com"
TIMEOUT_S: Final = 20.0
ATTEMPTS: Final = 3


class State(Enum):
    SUCCESS = "success"
    PENDING = "pending"
    FAILURE = "failure"
    ERROR = "error"


@dataclass(frozen=True)
class Comment:
    author: str
    updated_at: str
    body: str


@dataclass(frozen=True)
class Verdict:
    state: State
    message: str

    @property
    def passed(self) -> bool:
        return self.state is State.SUCCESS


@dataclass(frozen=True)
class Review:
    commit_sha: str | None
    status: str | None
    has_unresolved_findings: bool | None


class MarkerError(ValueError):
    pass


def parse_marker(body: str) -> Review | None:
    """The code review state of the last marker in `body`; None without a marker."""
    found = MARKER.findall(body)
    if not found:
        return None
    try:
        state = json.loads(found[-1])
    except json.JSONDecodeError as e:
        raise MarkerError(f"marker is not JSON: {e}") from e
    review = state.get("code_review") if isinstance(state, dict) else None
    if not isinstance(review, dict):
        raise MarkerError("marker has no code_review object")
    sha, status, unresolved = review.get("commit_sha"), review.get("status"), review.get("has_unresolved_findings")
    if not (sha is None or isinstance(sha, str)) or not (status is None or isinstance(status, str)):
        raise MarkerError("marker commit_sha/status are not strings")
    if not (unresolved is None or isinstance(unresolved, bool)):
        raise MarkerError("marker has_unresolved_findings is not a boolean")
    return Review(sha, status, unresolved)


def latest_marker(comments: list[Comment]) -> Comment | None:
    """The bot's most recently updated comment that carries a marker. Later list order breaks ties."""
    best: Comment | None = None
    for c in comments:
        if c.author == BOT and MARKER.search(c.body) and (best is None or c.updated_at >= best.updated_at):
            best = c
    return best


def decide(head_sha: str, comments: list[Comment]) -> Verdict:
    comment = latest_marker(comments)
    if comment is None:
        return Verdict(State.PENDING, "no Codex review yet; Codex reviews on open, or comment '@codex review'")
    try:
        review = parse_marker(comment.body)
    except MarkerError as e:
        return Verdict(State.FAILURE, f"Codex review state unreadable: {e}")
    if review is None:
        return Verdict(State.FAILURE, "Codex review state unreadable")
    short = head_sha[:10]
    if review.commit_sha != head_sha:
        seen = (review.commit_sha or "none")[:10]
        return Verdict(State.PENDING, f"Codex reviewed {seen}, head is {short}; comment '@codex review'")
    if review.status != "completed":
        return Verdict(State.PENDING, f"Codex review of {short} is {review.status or 'not started'}")
    if review.has_unresolved_findings is not False:
        return Verdict(State.FAILURE, f"Codex review of {short} has unresolved findings; fix, resolve, re-review")
    return Verdict(State.SUCCESS, f"Codex reviewed {short}: no unresolved findings")


def comments_from(issue_comments: list[dict[str, object]], reviews: list[dict[str, object]]) -> list[Comment]:
    """Issue comments (edited in place) and review bodies, as one list."""
    out: list[Comment] = []
    for raw, stamp in [(c, "updated_at") for c in issue_comments] + [(r, "submitted_at") for r in reviews]:
        user = raw.get("user")
        login = user.get("login") if isinstance(user, dict) else None
        body, when = raw.get("body"), raw.get(stamp)
        if isinstance(login, str) and isinstance(body, str) and isinstance(when, str):
            out.append(Comment(login, when, body))
    return out


class Api:
    def __init__(self, token: str | None) -> None:
        self.token = token

    def request(self, method: str, path: str, payload: dict[str, str] | None = None) -> object:
        data = json.dumps(payload).encode() if payload is not None else None
        headers = {"Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28",
                   "User-Agent": "oos-codex-gate"}
        if self.token:
            headers["Authorization"] = f"Bearer {self.token}"
        last: Exception | None = None
        for attempt in range(ATTEMPTS):
            req = urllib.request.Request(API + path, data=data, headers=headers, method=method)
            try:
                with urllib.request.urlopen(req, timeout=TIMEOUT_S) as r:
                    return json.loads(r.read() or b"null")
            except urllib.error.HTTPError as e:
                if e.code < 500:
                    raise
                last = e
            except urllib.error.URLError as e:
                last = e
            time.sleep(2 ** attempt)
        raise RuntimeError(f"{method} {path}: {last}")

    def pages(self, path: str) -> list[dict[str, object]]:
        out: list[dict[str, object]] = []
        for page in range(1, 51):
            got = self.request("GET", f"{path}?per_page=100&page={page}")
            if not isinstance(got, list):
                raise RuntimeError(f"GET {path}: expected a list")
            out.extend(x for x in got if isinstance(x, dict))
            if len(got) < 100:
                return out
        raise RuntimeError(f"GET {path}: more than 50 pages")


def head_of(pr: object) -> str:
    head = pr.get("head") if isinstance(pr, dict) else None
    sha = head.get("sha") if isinstance(head, dict) else None
    if not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise RuntimeError("pull request has no head sha")
    return sha


def publish(api: Api, repo: str, sha: str, verdict: Verdict, target_url: str | None) -> bool:
    payload = {"state": verdict.state.value, "context": CONTEXT, "description": verdict.message[:140]}
    if target_url:
        payload["target_url"] = target_url
    try:
        api.request("POST", f"/repos/{repo}/statuses/{sha}", payload)
    except urllib.error.HTTPError as e:
        # A fork PR's token is read-only: the issue_comment run (base repository context) publishes instead.
        print(f"::warning::cannot publish {CONTEXT} status ({e.code}); a comment event publishes it")
        return False
    return True


def report(sha: str, verdict: Verdict, status: bool = False) -> int:
    """Exit 0 on a pass. With `status` the commit status carries the verdict, so a pending review (none yet, or one
    still running) is no failure of the job either: a draft PR would otherwise show a red job until Codex reviews."""
    ok = verdict.passed or (status and verdict.state is State.PENDING)
    level = "notice" if ok else "error"
    print(f"::{level}::{CONTEXT} {verdict.state.value} for {sha}: {verdict.message}")
    return 0 if ok else 1


def load(path: Path) -> list[dict[str, object]]:
    got = json.loads(path.read_text())
    if not isinstance(got, list):
        raise RuntimeError(f"{path}: expected a JSON list")
    return [x for x in got if isinstance(x, dict)]


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="mode", required=True)
    for name in ("pr", "queue"):
        s = sub.add_parser(name)
        s.add_argument("--repo", required=True)
        s.add_argument("--publish", action="store_true")
        s.add_argument("--target-url")
        if name == "pr":
            s.add_argument("--pr", required=True, type=int)
        else:
            s.add_argument("--sha", required=True)
    sub.add_parser("offline").add_argument("--dir", required=True, type=Path)
    a = p.parse_args(argv)
    try:
        if a.mode == "offline":
            sha = head_of(json.loads((a.dir / "pr.json").read_text()))
            comments = comments_from(load(a.dir / "issue_comments.json"), load(a.dir / "reviews.json"))
            return report(sha, decide(sha, comments))
        if not re.fullmatch(r"[\w.-]+/[\w.-]+", a.repo):
            raise RuntimeError(f"bad repository {a.repo!r}")
        api = Api(os.environ.get("GITHUB_TOKEN") or os.environ.get("GH_TOKEN"))
        if a.mode == "queue":
            # Queue entry already required a passing codex-review on the PR head; the group commit inherits it.
            if not re.fullmatch(r"[0-9a-f]{40}", a.sha):
                raise RuntimeError(f"bad commit sha {a.sha!r}")
            sha, verdict = a.sha, Verdict(State.SUCCESS, "reviewed before the merge queue")
        else:
            sha = head_of(api.request("GET", f"/repos/{a.repo}/pulls/{a.pr}"))
            comments = comments_from(api.pages(f"/repos/{a.repo}/issues/{a.pr}/comments"),
                                     api.pages(f"/repos/{a.repo}/pulls/{a.pr}/reviews"))
            verdict = decide(sha, comments)
        if a.publish:
            publish(api, a.repo, sha, verdict, a.target_url)
        return report(sha, verdict, status=a.publish)
    except (RuntimeError, OSError, ValueError, urllib.error.HTTPError) as e:
        print(f"::error::{CONTEXT}: {e}")
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
