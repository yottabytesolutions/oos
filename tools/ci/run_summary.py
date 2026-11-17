#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Job and step durations of a workflow run as Markdown, for the job summary: where the pipeline spends its time.

  run_summary.py --repo OWNER/NAME --run ID [--attempt N] [--exclude JOB]

Appends to $GITHUB_STEP_SUMMARY when set, else prints. Needs `actions: read`. Exit status 0 even when the API is
unavailable: the summary informs, it never gates.
"""
import argparse
from dataclasses import dataclass
from datetime import datetime
import json
import os
import sys
import time
from typing import Final
import urllib.error
import urllib.request

API: Final = "https://api.github.com"
TIMEOUT_S: Final = 20.0
ATTEMPTS: Final = 3


@dataclass(frozen=True)
class Step:
    name: str
    seconds: int
    conclusion: str


@dataclass(frozen=True)
class Job:
    name: str
    conclusion: str
    queued: int
    seconds: int
    steps: tuple[Step, ...]


def stamp(value: object) -> datetime | None:
    if not isinstance(value, str) or not value or value.startswith("0001-"):
        return None
    return datetime.fromisoformat(value.replace("Z", "+00:00"))


def span(start: object, end: object) -> int | None:
    a, b = stamp(start), stamp(end)
    return int((b - a).total_seconds()) if a and b else None


def parse(jobs: list[dict[str, object]], exclude: str) -> list[Job]:
    out = []
    for j in jobs:
        if j.get("name") == exclude or j.get("status") != "completed":
            continue
        seconds = span(j.get("started_at"), j.get("completed_at"))
        if seconds is None:
            continue
        steps = []
        for s in j.get("steps") or []:
            if not isinstance(s, dict) or s.get("conclusion") == "skipped":
                continue
            d = span(s.get("started_at"), s.get("completed_at"))
            if d is not None:
                steps.append(Step(str(s.get("name")), d, str(s.get("conclusion"))))
        queued = span(j.get("created_at"), j.get("started_at")) or 0
        out.append(Job(str(j.get("name")), str(j.get("conclusion")), queued, seconds, tuple(steps)))
    return out


def clock(seconds: int) -> str:
    return f"{seconds // 60}:{seconds % 60:02d}"


def markdown(jobs: list[Job]) -> str:
    if not jobs:
        return "No completed jobs.\n"
    lines = ["| Job | Result | Queued | Duration |", "|---|---|---:|---:|"]
    for j in sorted(jobs, key=lambda j: -j.seconds):
        lines.append(f"| {j.name} | {j.conclusion} | {clock(j.queued)} | {clock(j.seconds)} |")
    for j in sorted(jobs, key=lambda j: j.name):
        steps = [s for s in j.steps if s.seconds > 0]
        if not steps:
            continue
        lines += ["", f"<details><summary>{j.name}: steps</summary>", "", "| Step | Duration |", "|---|---:|"]
        lines += [f"| {s.name} | {clock(s.seconds)} |" for s in steps]
        lines += ["", "</details>"]
    return "\n".join(lines) + "\n"


def jobs_of(repo: str, run: str, attempt: str, token: str) -> list[dict[str, object]]:
    headers = {"Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28",
               "User-Agent": "oos-ci-summary", "Authorization": f"Bearer {token}"}
    path = f"/repos/{repo}/actions/runs/{run}/attempts/{attempt}/jobs?per_page=100"
    last: Exception | None = None
    for i in range(ATTEMPTS):
        try:
            req = urllib.request.Request(API + path, headers=headers)
            with urllib.request.urlopen(req, timeout=TIMEOUT_S) as r:
                got = json.loads(r.read())
            jobs = got.get("jobs") if isinstance(got, dict) else None
            if not isinstance(jobs, list):
                raise RuntimeError(f"GET {path}: no job list")
            return [j for j in jobs if isinstance(j, dict)]
        except urllib.error.HTTPError as e:
            if e.code < 500:
                raise RuntimeError(f"GET {path}: {e}") from e
            last = e
        except (urllib.error.URLError, TimeoutError) as e:
            last = e
        time.sleep(2 ** i)
    raise RuntimeError(f"GET {path}: {last}")


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--run", required=True)
    parser.add_argument("--attempt", default="1")
    parser.add_argument("--exclude", default="", help="this job's name: it is still running")
    args = parser.parse_args(argv)
    try:
        jobs = jobs_of(args.repo, args.run, args.attempt, os.environ.get("GITHUB_TOKEN", ""))
    except RuntimeError as e:
        print(f"::warning::run summary unavailable: {e}")
        return 0
    text = "## Job and step durations\n\n" + markdown(parse(jobs, args.exclude))
    out = os.environ.get("GITHUB_STEP_SUMMARY")
    if out:
        with open(out, "a", encoding="utf-8") as f:
            f.write(text)
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
