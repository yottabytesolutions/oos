#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""PR metadata rules (docs/process/ci.md). Errors fail the `pr-meta` check; warnings only annotate.

  pr_meta.py pr        branch, title, DCO, runner labels, size, task link; reads the PR from the environment
  pr_meta.py runners   only the runner labels of .github/workflows (merge queue and push)

Environment for `pr`: PR_HEAD_REF, PR_TITLE, PR_BODY, PR_BASE_SHA, PR_HEAD_SHA, PR_AUTHOR, PR_LABELS (JSON list of
names). Commits and the diff come from the local clone (fetch-depth 0).
"""
from dataclasses import dataclass, field
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
from typing import Final

TYPES: Final = ("feat", "fix", "perf", "refactor", "docs", "test", "ci", "build", "chore")
BRANCH: Final = re.compile(rf"(?:{'|'.join(TYPES)})/(?:\d+|t\d{{4}})-[a-z0-9]+(?:-[a-z0-9]+)*")
TITLE: Final = re.compile(rf"(?:{'|'.join(TYPES)})(?:\([a-z0-9][a-z0-9._/-]*\))?!?: \S.*")
TITLE_MAX: Final = 72
SIZE_WARN: Final = 400
# Generated or recorded test data: reviewed by its own diff tooling, not by line count.
SIZE_EXEMPT_DIRS: Final = ("golden", "fixtures")
SIZE_EXEMPT_PREFIXES: Final = ("xtask/data/",)
SIZE_EXEMPT_NAMES: Final = ("Cargo.lock",)
# Standard GitHub-hosted runners only: free for a public repository. Larger runners (`ubuntu-24.04-16core`,
# `*-xlarge`, any custom label) are billed per minute even here, and self-hosted runners would run fork code.
RUNNERS: Final = frozenset({"ubuntu-24.04", "ubuntu-24.04-arm", "ubuntu-latest"})
DEPENDABOT: Final = "dependabot[bot]"
AGENT_APP: Final = "yottabyte-oos-agent[bot]"
SIGNOFF: Final = re.compile(r"^Signed-off-by: [^<>\n]+ <([^<>\s]+)>\s*$", re.MULTILINE)
AGENT_DETAIL: Final = re.compile(
    r"^(?:Claude-Session:|Agent:|Co-authored-by: [^\n]*(?:anthropic|claude|openai|codex|copilot|cursor|chatgpt))",
    re.MULTILINE | re.IGNORECASE)


@dataclass
class Result:
    errors: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)


@dataclass(frozen=True)
class Commit:
    sha: str
    author_email: str
    author_name: str
    message: str


def check_branch(ref: str, author: str) -> list[str]:
    if author == DEPENDABOT and ref.startswith("dependabot/"):
        return []
    if BRANCH.fullmatch(ref):
        return []
    return [f"branch {ref!r}: expected <type>/<ref>-<slug>, type one of {', '.join(TYPES)}, "
            "ref an issue number or t + four digits (feat/123-scope-trigger, fix/t0075-normal-trigger)"]


def check_title(title: str) -> list[str]:
    errors = []
    if not TITLE.fullmatch(title):
        errors.append(f"title {title!r}: expected Conventional Commits 'type(scope): summary', "
                      f"type one of {', '.join(TYPES)}")
    if len(title) > TITLE_MAX:
        errors.append(f"title is {len(title)} characters, at most {TITLE_MAX}: it becomes the squash commit subject")
    return errors


def check_commits(commits: list[Commit], pr_author: str) -> Result:
    """DCO on every commit. Only Dependabot's own commits in Dependabot's own PR are exempt (it cannot sign off)."""
    r = Result()
    if not commits:
        r.errors.append("no commits between base and head")
    for c in commits:
        short = c.sha[:10]
        if pr_author == DEPENDABOT and c.author_name == DEPENDABOT:
            continue
        emails = [e.lower() for e in SIGNOFF.findall(c.message)]
        if not emails:
            r.errors.append(f"{short}: no 'Signed-off-by:' trailer (DCO); commit with `git commit -s`")
        elif c.author_email.lower() not in emails:
            r.errors.append(f"{short}: Signed-off-by does not match the author email; commit with `git commit -s`")
        if AGENT_DETAIL.search(c.message):
            r.warnings.append(f"{short}: commit message carries tool details (session, Agent or AI co-author "
                              "trailer); commits carry only the DCO sign-off")
    return r


def check_runners(workflows: dict[str, str]) -> list[str]:
    """Every `runs-on` is a literal label from RUNNERS: no expressions, lists, groups, larger or self-hosted runners."""
    errors = []
    for name, text in sorted(workflows.items()):
        for number, line in enumerate(text.splitlines(), 1):
            m = re.match(r"\s*(?:-\s+)?runs-on:\s*(.*?)\s*(?:#.*)?$", line)
            if not m:
                continue
            value = m.group(1).strip("'\"")
            if value not in RUNNERS:
                errors.append(f"{name}:{number}: runs-on {m.group(1)!r}; only {', '.join(sorted(RUNNERS))} "
                              "(standard GitHub-hosted) are allowed, no larger or self-hosted runners")
    return errors


def counted(path: str) -> bool:
    p = PurePosixPath(path)
    return not (any(d in p.parts[:-1] for d in SIZE_EXEMPT_DIRS) or path.startswith(SIZE_EXEMPT_PREFIXES)
                or p.name in SIZE_EXEMPT_NAMES)


def changed_lines(numstat: str) -> int:
    total = 0
    for line in numstat.splitlines():
        parts = line.split("\t", 2)
        if len(parts) != 3 or parts[0] == "-":
            continue
        if counted(parts[2]):
            total += int(parts[0]) + int(parts[1])
    return total


def check_size(lines: int) -> list[str]:
    if lines > SIZE_WARN:
        return [f"{lines} changed lines (goldens, fixtures, Cargo.lock and ratchet data excluded), more than "
                f"{SIZE_WARN}: split the PR if it holds more than one topic"]
    return []


def check_link(body: str) -> list[str]:
    if re.search(r"(?:^|[\s(])#\d+\b|docs/tasks/\d{4}-", body):
        return []
    return ["PR text links no issue (#NNN) and no docs/tasks file"]


def check_agent(author: str, labels: list[str]) -> list[str]:
    if (author == AGENT_APP) != ("agent" in labels):
        return [f"author {author} and the 'agent' label disagree: agent PRs come from {AGENT_APP} and carry 'agent'"]
    return []


def git(*args: str) -> str:
    return subprocess.run(["git", *args], check=True, capture_output=True, text=True, timeout=60).stdout


def commits_between(base: str, head: str) -> list[Commit]:
    raw = git("log", "--no-merges", "--format=%H%x1f%ae%x1f%an%x1f%B%x1e", f"{base}..{head}")
    out = []
    for record in raw.split("\x1e"):
        record = record.strip("\n")
        if not record:
            continue
        sha, email, name, message = record.split("\x1f", 3)
        out.append(Commit(sha, email, name, message))
    return out


def workflows(root: Path) -> dict[str, str]:
    d = root / ".github/workflows"
    return {str(p.relative_to(root)): p.read_text() for p in sorted(d.glob("*.y*ml"))}


def annotate(r: Result) -> int:
    for w in r.warnings:
        print(f"::warning::{w}")
    for e in r.errors:
        print(f"::error::{e}")
    print(f"pr-meta: {len(r.errors)} error(s), {len(r.warnings)} warning(s)")
    return 1 if r.errors else 0


def env(name: str) -> str:
    value = os.environ.get(name)
    if value is None:
        raise KeyError(f"{name} is not set")
    return value


def main(argv: list[str]) -> int:
    root = Path(git("rev-parse", "--show-toplevel").strip())
    r = Result(errors=check_runners(workflows(root)))
    if argv == ["runners"]:
        return annotate(r)
    if argv != ["pr"]:
        print(__doc__)
        return 2
    try:
        author, labels = env("PR_AUTHOR"), json.loads(env("PR_LABELS"))
        base, head = env("PR_BASE_SHA"), env("PR_HEAD_SHA")
        if not isinstance(labels, list) or not all(isinstance(x, str) for x in labels):
            raise ValueError("PR_LABELS is not a JSON list of names")
        r.errors += check_branch(env("PR_HEAD_REF"), author) + check_title(env("PR_TITLE"))
        found = check_commits(commits_between(base, head), author)
        r.errors += found.errors
        r.warnings += found.warnings
        r.warnings += check_size(changed_lines(git("diff", "--numstat", f"{base}...{head}")))
        r.warnings += check_link(os.environ.get("PR_BODY", "")) + check_agent(author, labels)
    except (KeyError, ValueError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as e:
        r.errors.append(f"input: {e}")
    return annotate(r)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
