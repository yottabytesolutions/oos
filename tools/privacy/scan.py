#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Scan staged additions, commit messages, incoming commits or tracked files."""

import argparse
import json
import re
import subprocess
import sys
from collections.abc import Iterable, Iterator
from pathlib import Path

from git_input import Record, git, incoming, staged, text_lines
from patterns import SESSION_TRAILER, SIGNOFF_TRAILER, matches
from policy import digest, load_allowlist, load_literals


def findings(records: Iterable[Record], allowed: set[tuple[str, str, str]],
             literals: list[str]) -> Iterator[tuple[str, int, str]]:
    for name, number, line, is_message in records:
        rules = {rule for rule, value in matches(line)
                 if not (is_message and rule == "session-url" and SESSION_TRAILER.fullmatch(line))
                 and not (is_message and rule == "email" and SIGNOFF_TRAILER.fullmatch(line))
                 and (is_message or (name, rule, digest(value)) not in allowed)}
        if any(literal in line for literal in literals):
            rules.add("local-literal")  # Private literals can never be allowlisted.
        for rule in sorted(rules):
            yield name, number, rule


def safe_name(name: str, literals: list[str]) -> str:
    for _, value in matches(name):
        name = name.replace(value, "<redacted>")
    for literal in literals:
        name = name.replace(literal, "<redacted>")
    return json.dumps(name, ensure_ascii=True)


def working_tree(root: Path) -> Iterator[Record]:
    for raw_name in git(root, "ls-files", "--cached", "--others", "--exclude-standard", "-z").split(b"\0"):
        if not raw_name:
            continue
        name = raw_name.decode("utf-8", errors="surrogateescape")
        path = root / name
        if path.is_symlink():
            text = str(path.readlink())
        elif path.is_file():
            text = path.read_bytes().decode("utf-8", errors="replace")
        else:
            continue  # Deleted files and submodules have no working-tree text.
        yield name, 0, name, False
        for number, line in enumerate(text_lines(text), 1):
            yield name, number, line, False


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    mode = result.add_mutually_exclusive_group(required=True)
    mode.add_argument("--staged", action="store_true")
    mode.add_argument("--message", type=Path, metavar="FILE")
    mode.add_argument("--range", nargs=2, metavar=("BASE", "HEAD"))
    mode.add_argument("--tree", action="store_true", help="audit tracked and nonignored new working-tree files")
    result.add_argument("--allowlist", type=Path, help="trusted policy file; CI uses the base revision")
    result.add_argument("--git-message", action="store_true", help="strip Git comments and scissors from --message")
    result.add_argument("--exclude-remotes", action="append", default=[], metavar="PATTERN",
                        help="exclude commits reachable from matching remote refs, e.g. upstream/*")
    return result


def comment_char(root: Path, text: str) -> str | None:
    result = subprocess.run(["git", "-C", str(root), "config", "core.commentChar"],
                            capture_output=True, check=False)
    if result.returncode == 1:
        return "#"  # Git's default when the setting is absent.
    if result.returncode:
        raise ValueError("cannot read Git comment character")
    marker = result.stdout.decode("utf-8").strip()
    if marker == "auto":
        # Git picks a marker before appending its template, so recomputing it from
        # the finished message would pick a different character. Scissors identify
        # the chosen marker; otherwise retain ambiguous lines to fail closed.
        scissors = re.search(r"^([#;@!$%^&|:]) ------------------------ >8 ------------------------$", text, re.M)
        return scissors[1] if scissors else None
    if len(marker) != 1:
        raise ValueError("invalid Git comment character")
    return marker


def message_lines(text: str, cleanup: bool, marker: str | None = "#") -> Iterator[tuple[int, str]]:
    for number, line in enumerate(text_lines(text), 1):
        if cleanup and marker and line == marker + " ------------------------ >8 ------------------------":
            break
        if not cleanup or marker is None or not line.startswith(marker):
            yield number, line


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    try:
        if (args.git_message and not args.message) or (args.exclude_remotes and not args.range):
            raise ValueError("message cleanup and remote exclusions require their corresponding input modes")
        root = Path(git(Path.cwd(), "rev-parse", "--show-toplevel").decode().strip())
        policy_path = args.allowlist or root / "tools/privacy/allowlist.json"
        allowed = load_allowlist(policy_path)
        literal_path = root / "local/privacy-literals.txt"
        indexed = git(root, "ls-files", "--", "local/privacy-literals.txt")
        if indexed:
            raise ValueError("local privacy literals must remain untracked")
        literals = load_literals(literal_path)
        if args.staged:
            records = staged(root)
        elif args.range:
            records = incoming(root, *args.range, args.exclude_remotes)
        elif args.message:
            text = args.message.read_bytes().decode("utf-8")
            marker = comment_char(root, text) if args.git_message else None
            records = (("commit-message", n, line, True)
                       for n, line in message_lines(text, args.git_message, marker))
        else:
            records = working_tree(root)
        count = 0
        for name, number, rule in findings(records, allowed, literals):
            # JSON quoting prevents paths from injecting terminal escapes or workflow commands.
            print(f"privacy: {safe_name(name, literals)}:{number}: {rule} [redacted]")
            count += 1
        print(f"privacy: {count} finding(s)")
        return 1 if count else 0
    except (OSError, ValueError, subprocess.SubprocessError):
        print("privacy: input or policy error; check Git revisions, policy and file permissions [redacted]", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
