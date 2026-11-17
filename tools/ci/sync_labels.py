#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""Create or update the labels of .github/labels.yml on a repository with `gh label create --force`.

  sync_labels.py OWNER/NAME [--dry-run]

Labels not in the file stay untouched. labels.yml is a flat list of name/color/description mappings; this reads
exactly that subset, so no YAML library is needed.
"""
from dataclasses import dataclass
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class Label:
    name: str
    color: str
    description: str


def parse(text: str) -> list[Label]:
    labels: list[Label] = []
    item: dict[str, str] = {}

    def close() -> None:
        if not item:
            return
        if set(item) != {"name", "color", "description"} or not re.fullmatch(r"[0-9a-f]{6}", item["color"]):
            raise ValueError(f"label entry {item}: needs name, 6-digit hex color and description")
        labels.append(Label(item["name"], item["color"], item["description"]))
        item.clear()

    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        m = re.fullmatch(r"(- |  )(name|color|description): (.+)", line)
        if not m:
            raise ValueError(f"labels.yml:{number}: unsupported line {line!r}")
        if m.group(1) == "- ":
            close()
        elif not item:
            raise ValueError(f"labels.yml:{number}: field outside an entry")
        item[m.group(2)] = m.group(3).strip()
    close()
    names = [x.name for x in labels]
    if len(set(names)) != len(names):
        raise ValueError("labels.yml: duplicate names")
    return labels


def main(argv: list[str]) -> int:
    if len(argv) not in (1, 2) or (len(argv) == 2 and argv[1] != "--dry-run"):
        print(__doc__)
        return 2
    repo = argv[0]
    for label in parse((ROOT / ".github/labels.yml").read_text()):
        cmd = ["gh", "label", "create", label.name, "--repo", repo, "--color", label.color,
               "--description", label.description, "--force"]
        print(" ".join(cmd[:4]), label.color)
        if len(argv) == 1:
            subprocess.run(cmd, check=True, timeout=60)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
