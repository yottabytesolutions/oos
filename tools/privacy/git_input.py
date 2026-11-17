# SPDX-License-Identifier: GPL-2.0-or-later
"""Read Git objects without staging, creating commits or touching the device."""

import re
import subprocess
from collections.abc import Iterator
from pathlib import Path

Record = tuple[str, int, str, bool]  # Path/label, line number, text, commit-message source.


def text_lines(text: str) -> Iterator[str]:
    # Git counts LF-delimited lines. Other Unicode/control separators are content.
    lines = text.split("\n")
    if lines[-1] == "":
        lines.pop()  # A terminal LF ends a line; it does not create another one.
    yield from (line.removesuffix("\r") for line in lines)


def git(root: Path, *args: str) -> bytes:
    result = subprocess.run(["git", "-C", str(root), *args], capture_output=True, check=False)
    if result.returncode:
        raise ValueError("Git input failed; check the repository and requested revisions")
    return result.stdout


def resolve(root: Path, revision: str) -> str:
    # --end-of-options prevents user-provided revisions from becoming Git options.
    return git(root, "rev-parse", "--verify", "--end-of-options", revision + "^{commit}").decode().strip()


def entries(root: Path, revision: str | None) -> Iterator[tuple[str, str]]:
    if revision is None:
        records = git(root, "ls-files", "--stage", "-z")
    else:
        records = git(root, "ls-tree", "-r", "-z", revision)
    for record in records.split(b"\0"):
        if not record:
            continue
        metadata, name = record.split(b"\t", 1)
        fields = metadata.split()
        mode = fields[0]
        if mode == b"160000":
            continue  # submodule commits are not file contents
        if revision is None and fields[2] != b"0":
            raise ValueError("resolve unmerged index entries before scanning")
        yield name.decode("utf-8", errors="surrogateescape"), fields[2 if revision else 1].decode()


def additions(patch: bytes) -> Iterator[tuple[int, str]]:
    line_number = 0
    in_hunk = False
    for line in text_lines(patch.decode("utf-8", errors="replace")):
        header = re.match(r"@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@", line)
        if header:
            line_number = int(header[1])
            in_hunk = True
        elif in_hunk and line.startswith("+"):
            yield line_number, line[1:]
            line_number += 1
        elif line.startswith(" "):
            line_number += 1


def blob_additions(root: Path, previous: str | None, oid: str) -> Iterator[tuple[int, str]]:
    if previous is None:
        data = git(root, "cat-file", "blob", oid).decode("utf-8", errors="replace")
        yield from enumerate(text_lines(data), 1)
    else:
        patch = git(root, "diff", "--no-ext-diff", "--no-textconv", "--text", "--unified=0", previous, oid)
        yield from additions(patch)


def changed(root: Path, old: dict[str, str], revision: str | None) -> Iterator[Record]:
    # Compare blobs individually: unusual filenames and binary attributes cannot hide added text.
    for name, oid in entries(root, revision):
        previous = old.get(name)
        if oid == previous:
            continue
        yield name, 0, name, False
        for number, line in blob_additions(root, previous, oid):
            yield name, number, line, False


def merge_changed(root: Path, parents: list[str], revision: str) -> Iterator[Record]:
    old = [dict(entries(root, parent)) for parent in parents]
    for name, oid in entries(root, revision):
        previous = [tree.get(name) for tree in old]
        if oid in previous:
            continue  # A result identical to any parent adds nothing of its own.
        yield name, 0, name, False
        # Intersect result line numbers, preserving duplicate lines and their locations.
        # Only additions against every parent belong to the merge's resolution.
        added = dict(blob_additions(root, previous[0], oid))
        for parent_oid in previous[1:]:
            if not added:
                break
            numbers = {number for number, _ in blob_additions(root, parent_oid, oid)}
            added = {number: line for number, line in added.items() if number in numbers}
        for number, line in added.items():
            yield name, number, line, False


def staged(root: Path) -> Iterator[Record]:
    head = subprocess.run(["git", "-C", str(root), "rev-parse", "--verify", "--quiet", "HEAD"],
                          capture_output=True, check=False)
    if head.returncode not in (0, 1):
        raise ValueError("cannot resolve staged base")
    old = dict(entries(root, resolve(root, "HEAD"))) if head.returncode == 0 else {}
    yield from changed(root, old, None)


def incoming(root: Path, base: str, head: str,
             exclude_remotes: list[str] | None = None) -> Iterator[Record]:
    start, end = resolve(root, base), resolve(root, head)
    exclusions = ["--remotes=" + pattern for pattern in (exclude_remotes or [])]
    revisions = text_lines(git(root, "rev-list", "--reverse", f"{start}..{end}",
                               *(["--not", *exclusions] if exclusions else [])).decode())
    for revision in revisions:
        message = git(root, "show", "-s", "--format=%B", revision).decode("utf-8", errors="replace")
        for number, line in enumerate(text_lines(message), 1):
            yield f"commit:{revision[:12]}", number, line, True
        parents = git(root, "rev-list", "--parents", "-n", "1", revision).decode().split()[1:]
        if len(parents) > 1:
            yield from merge_changed(root, parents, revision)
        else:
            old = dict(entries(root, parents[0])) if parents else {}
            yield from changed(root, old, revision)
