#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-or-later
"""The production code section of the release notes (ADR 0035). Fails when production code grew since the previous
release and neither the tag annotation nor the release's CHANGELOG.md section has a `Code growth:` paragraph.

  code_growth.py --loc loc.json --table loc.md --version 0.2.1 --tag-message tag.txt --changelog CHANGELOG.md

`loc.json` and `loc.md` are `cargo xtask loc --compare <previous tag>` with and without `--json`.
"""
import argparse
import json
from pathlib import Path
import re
import sys

MARK = "Code growth:"


def section(changelog: str, version: str) -> str:
    """The CHANGELOG.md section of `version`: from its `## <version>` heading to the next `## ` heading."""
    heading = re.compile(rf"## {re.escape(version)}(?::|\s|$)")
    out, inside = [], False
    for line in changelog.splitlines():
        if line.startswith("## "):
            inside = bool(heading.match(line))
        elif inside:
            out.append(line)
    return "\n".join(out)


def reason(*texts: str) -> str | None:
    """The first paragraph that starts with `Code growth:` and gives a reason after it, joined to one line."""
    for text in texts:
        for paragraph in re.split(r"\n\s*\n", text):
            line = " ".join(paragraph.split())
            if line.startswith(MARK) and line[len(MARK):].strip():
                return line
    return None


def notes(loc: dict, table: str, why: str | None) -> tuple[str, bool]:
    """The release notes section and whether the release may go on."""
    delta = loc["production_total"].get("delta")
    lines = ["## Production code", ""]
    if loc.get("base"):
        lines += [f"Compared with {loc['base']} (ADR 0035).", ""]
    lines.append(table.rstrip())
    if why:
        lines += ["", why]
    return "\n".join(lines) + "\n", not (delta and delta > 0) or why is not None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    for name in ("--loc", "--table", "--tag-message", "--changelog"):
        parser.add_argument(name, type=Path, required=True)
    parser.add_argument("--version", required=True)
    a = parser.parse_args()
    loc = json.loads(a.loc.read_text(encoding="utf-8"))
    why = reason(a.tag_message.read_text(encoding="utf-8"), section(a.changelog.read_text(encoding="utf-8"), a.version))
    text, ok = notes(loc, a.table.read_text(encoding="utf-8"), why)
    sys.stdout.write(text)
    if not ok:
        delta = loc["production_total"]["delta"]
        print(f"::error::production code grew by {delta} lines since {loc.get('base')}: add a paragraph that starts "
              f"with '{MARK}' to the tag annotation or to the {a.version} section of CHANGELOG.md (ADR 0035)",
              file=sys.stderr)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
