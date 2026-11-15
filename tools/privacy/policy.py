"""Strict, exact-value exceptions and optional untracked literal strings."""

import hashlib
import json
from pathlib import Path

from git_input import text_lines
from patterns import RULES

RULE_NAMES = set(RULES) | {"mac", "private-ipv4", "private-ipv6", "global-ipv6"}


def digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def load_allowlist(path: Path) -> set[tuple[str, str, str]]:
    entries = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(entries, list):
        raise ValueError("allowlist must be a JSON array")
    result: set[tuple[str, str, str]] = set()
    for entry in entries:
        if not isinstance(entry, dict) or set(entry) != {"path", "rule", "sha256", "reason"}:
            raise ValueError("allowlist entry needs path, rule, sha256 and reason")
        if not all(isinstance(value, str) and value.strip() for value in entry.values()):
            raise ValueError("allowlist fields must be nonempty strings")
        name = entry["path"]
        if name.startswith("/") or ".." in Path(name).parts or any(c in name for c in "*?["):
            raise ValueError("allowlist paths must be exact repository-relative paths")
        if entry["rule"] not in RULE_NAMES:
            raise ValueError("unknown allowlist rule")
        if len(entry["sha256"]) != 64 or any(c not in "0123456789abcdef" for c in entry["sha256"]):
            raise ValueError("invalid allowlist hash")
        key = (name, entry["rule"], entry["sha256"])
        if key in result:
            raise ValueError("duplicate allowlist entry")
        result.add(key)
    return result


def load_literals(path: Path) -> list[str]:
    if not path.exists():
        return []
    # One exact, case-sensitive value per line; no stripping of meaningful spaces.
    return [line for line in text_lines(path.read_bytes().decode("utf-8")) if line and not line.startswith("#")]
