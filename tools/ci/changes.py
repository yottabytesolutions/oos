#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Whether a job of .github/workflows/pr.yml has work to do. Prints `run=true|false` (and appends it to
$GITHUB_OUTPUT when set). The job keeps its required name either way; only its build and test steps are skipped.

  changes.py --scope rust|all --check NAME

- `rust` jobs (clippy, tests, deny) skip when the change touches only documents, GitHub settings and the CI scripts
  (SKIP below). Anything else, and every file a Rust source includes, runs them.
- In a merge-queue group whose tree equals the head of its pull request, a job whose check already passed on that
  head skips: the same tree under the same workflow gives the same result.
- Every other event (push to main, nightly) runs everything. When the base, the diff or the API is not available the
  job runs: an unknown change is a Rust change.

Environment: EVENT (github.event_name), BASE (base commit of the PR or group), QUEUE_REF (merge_group.head_ref),
REPO (owner/name), GITHUB_TOKEN (needs checks: read and pull-requests: read for the queue rule).
"""
import argparse
from dataclasses import dataclass
import json
import os
from pathlib import PurePosixPath
import re
import subprocess
import sys
import time
from typing import Final
import urllib.error
import urllib.parse
import urllib.request

# Paths that no clippy, test or cargo-deny result depends on.
SKIP_DIRS: Final = ("docs/", ".github/", "tools/ci/")
SKIP_SUFFIXES: Final = (".md",)
# Exceptions inside SKIP_DIRS: the workflow that defines these jobs, this script, and files Rust tests include.
RUST_FILES: Final = frozenset({
    ".github/workflows/pr.yml",
    ".github/workflows/image.yml",  # fpgad/tests/explicit_signing.rs includes it
    "tools/ci/changes.py",
})
QUEUE_REF: Final = re.compile(r"^refs/heads/gh-readonly-queue/[^/]+(?:/[^/]+)*/pr-(\d+)-[0-9a-f]{40}$")
API: Final = "https://api.github.com"
TIMEOUT_S: Final = 20.0
ATTEMPTS: Final = 3


@dataclass(frozen=True)
class Decision:
    run: bool
    reason: str


def needs_rust(path: str) -> bool:
    if path in RUST_FILES:
        return True
    if path.startswith(SKIP_DIRS):
        return False
    return not PurePosixPath(path).name.endswith(SKIP_SUFFIXES)


def by_paths(paths: list[str]) -> Decision:
    if not paths:
        return Decision(True, "empty diff: running")
    rust = [p for p in paths if needs_rust(p)]
    if rust:
        return Decision(True, f"{len(rust)} of {len(paths)} changed files need the Rust checks, e.g. {rust[0]}")
    return Decision(False, f"all {len(paths)} changed files are documents, GitHub settings or CI scripts")


def queue_pr(ref: str) -> int | None:
    m = QUEUE_REF.match(ref)
    return int(m.group(1)) if m else None


def passed(check_runs: object, name: str) -> bool:
    runs = check_runs.get("check_runs") if isinstance(check_runs, dict) else None
    if not isinstance(runs, list):
        return False
    return any(isinstance(r, dict) and r.get("name") == name and r.get("conclusion") == "success"
               and isinstance(r.get("app"), dict) and r["app"].get("slug") == "github-actions" for r in runs)


def get(path: str, token: str) -> object:
    headers = {"Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28",
               "User-Agent": "oos-ci-changes", "Authorization": f"Bearer {token}"}
    last: Exception | None = None
    for attempt in range(ATTEMPTS):
        try:
            with urllib.request.urlopen(urllib.request.Request(API + path, headers=headers), timeout=TIMEOUT_S) as r:
                return json.loads(r.read() or b"null")
        except urllib.error.HTTPError as e:
            if e.code < 500:
                raise RuntimeError(f"GET {path}: {e}") from e
            last = e
        except (urllib.error.URLError, TimeoutError) as e:
            # a read can time out after the connection succeeded: TimeoutError, not URLError
            last = e
        time.sleep(2 ** attempt)
    raise RuntimeError(f"GET {path}: {last}")


def git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True, timeout=60).stdout.strip()


def same_as_pr_head(repo: str, ref: str, check: str, token: str) -> Decision | None:
    """Skip when the group tree is the PR head tree and `check` passed on that head; None when it does not apply."""
    number = queue_pr(ref)
    if number is None:
        return None
    pr = get(f"/repos/{repo}/pulls/{number}", token)
    head = pr.get("head", {}).get("sha") if isinstance(pr, dict) else None
    if not isinstance(head, str):
        return None
    commit = get(f"/repos/{repo}/git/commits/{head}", token)
    tree = commit.get("tree", {}).get("sha") if isinstance(commit, dict) else None
    if tree != git("rev-parse", "HEAD^{tree}"):
        return None
    runs = f"/repos/{repo}/commits/{head}/check-runs?check_name={urllib.parse.quote(check)}&filter=latest"
    if not passed(get(runs, token), check):
        return None
    return Decision(False, f"group tree equals PR #{number} head {head[:10]}, where {check!r} passed")


def decide(scope: str, check: str, env: dict[str, str]) -> Decision:
    event = env.get("EVENT", "")
    if event not in ("pull_request", "merge_group"):
        return Decision(True, f"{event or 'unknown'} event: running everything")
    d = Decision(True, "all jobs run")
    if scope == "rust":
        base = env.get("BASE", "")
        if not base:
            return Decision(True, "no base commit: running")
        try:
            d = by_paths([p for p in git("diff", "--name-only", f"{base}...HEAD").splitlines() if p])
        except (subprocess.CalledProcessError, subprocess.TimeoutExpired) as e:
            return Decision(True, f"diff against {base[:10]} failed ({e}): running")
    if d.run and event == "merge_group":
        try:
            same = same_as_pr_head(env.get("REPO", ""), env.get("QUEUE_REF", ""), check, env.get("GITHUB_TOKEN", ""))
        except (RuntimeError, OSError, ValueError, subprocess.SubprocessError) as e:
            return Decision(True, f"queue shortcut unavailable ({e}): running")
        if same:
            return same
    return d


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--scope", choices=("rust", "all"), required=True)
    parser.add_argument("--check", required=True, help="the job's check name, as on the PR head")
    args = parser.parse_args(argv)
    d = decide(args.scope, args.check, dict(os.environ))
    line = f"run={'true' if d.run else 'false'}"
    print(f"{line} ({d.reason})")
    out = os.environ.get("GITHUB_OUTPUT")
    if out:
        with open(out, "a", encoding="utf-8") as f:
            f.write(line + "\n")
    if not d.run:
        print(f"::notice::{args.check}: skipped, {d.reason}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
