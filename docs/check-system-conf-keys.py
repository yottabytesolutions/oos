# SPDX-License-Identifier: GPL-2.0-or-later

"""Compare the documented key rows with the UI's Rust configuration parsers."""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parent.parent
UI = ROOT / "ui/src"
KEY = r'"([a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)*)"'


def source(path):
    text = path.read_text(encoding="utf-8").split("#[cfg(test)]\nmod tests {", 1)[0]
    return re.sub(r"(?m)^\s*//[^\n]*", "", text)


def inventory():
    paths = [UI / "sysconf.rs", *sorted((UI / "sysconf").rglob("*.rs"))]
    delegates = (
        "board/dho/switches.rs",
        "board/dho/hw.rs",
        "board/dho/hw/comparator.rs",
        "board/dho/hw/trigger.rs",
        "compress.rs",
        "drawpipe/funding.rs",
        "net6.rs",
        "sparse.rs",
        "system/clocks.rs",
        "system/memory.rs",
    )
    paths.extend(UI / name for name in delegates)
    keys, ignored = set(), set()
    for path in paths:
        text = source(path)
        keys.update(re.findall(r'\bconf_value\([^;]*?,\s*conf,\s*' + KEY + r'\s*,?\s*\)', text))
        if path == UI / "sysconf.rs":
            arms = text.split("let ok = match k {", 1)[1].split("\n                _ =>", 1)[0]
            for line in arms.splitlines():
                match = re.match(r'\s+("[a-z][a-z0-9_.]*"(?:\s*\|\s*"[a-z][a-z0-9_.]*")*)\s+(?:if |=>)', line)
                if match and line.startswith(" " * 16 + '"'):
                    # Bare success arms consume retired options without applying a setting.
                    target = ignored if line.endswith("=> true,") else keys
                    target.update(re.findall(KEY, match[1]))
            keys.update(re.findall(r"\bnetacl::value\([^,]+,\s*" + KEY, text))
        elif path == UI / "sysconf/switches/source.rs":
            keys.update(re.findall(r"\benabled\(" + KEY + r',\s*"OOS_', text))
        elif path == UI / "drawpipe/funding.rs":
            keys.update(re.findall(r'"(f2\.[a-z0-9_]+)"', text))
        elif path == UI / "system/clocks.rs":
            keys.update(re.findall(r"\bon\(" + KEY, text))
        elif path == UI / "system/memory.rs":
            keys.update(re.findall(r"\bmb\(" + KEY, text))
        elif path == UI / "board/dho/hw/comparator.rs":
            suffixes = re.search(r"\bconst MASK_MAP_KEYS:.*?=\s*\[(.*?)\];", text, re.S)[1]
            keys.update("mask.map." + suffix for suffix in re.findall(KEY, suffixes))
    return keys - ignored


def main():
    doc = (ROOT / "docs/system-conf-keys.md").read_text(encoding="utf-8")
    documented = set(re.findall(r"(?m)^\| `([a-z][a-z0-9_.]*)` \|", doc))
    parsed = inventory()
    missing = sorted(parsed - documented)
    extra = sorted(documented - parsed)
    if missing:
        print("Missing key rows:", ", ".join(missing), file=sys.stderr)
    if extra:
        print("Rows without parser matches:", ", ".join(extra), file=sys.stderr)
    if missing or extra:
        return 1
    print(f"Documented {len(parsed)} UI system.conf keys")
    return 0


if __name__ == "__main__":
    sys.exit(main())
