# SPDX-License-Identifier: GPL-2.0-or-later
"""Generic privacy patterns. Findings never contain the matched text."""

import ipaddress
import re
from collections.abc import Iterator


RULES = {
    "home-path": re.compile(r"(?:/(?:Users|home)/[^/\s\"'<>]+|[A-Za-z]:[\\/]Users[\\/][^\\/\s\"'<>]+)"),
    "email": re.compile(r"(?<![\w.+-])[\w.+-]+@(?:[\w-]+\.)+[A-Za-z]{2,}\b"),
    "serial": re.compile(r"\b(?:DHO|DS|MSO)[0-9][A-Z0-9]{7,23}\b", re.IGNORECASE),
    "private-key": re.compile(r"-----BEGIN (?:(?:RSA |EC |DSA |OPENSSH |ENCRYPTED )?PRIVATE KEY|PGP PRIVATE KEY BLOCK)-----"),
    "token": re.compile(
        r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|"
        r"(?:sk|sk-proj|sk-ant|glpat)-[A-Za-z0-9_-]{20,}|xox[baprs]-[A-Za-z0-9-]{20,}|"
        r"(?:AKIA|ASIA)[A-Z0-9]{16})\b"
    ),
    "session-url": re.compile(
        r"https?://(?:chatgpt\.com/(?:c|share|codex/tasks)/|chat\.openai\.com/(?:c|share)/|"
        r"claude\.ai/(?:(?:chat|share)/|code/session_))[^\s<>\"')]+", re.IGNORECASE
    ),
}
SESSION_TRAILER = re.compile(r"Claude-Session: https://claude\.ai/code/session_[A-Za-z0-9_-]+")
# DCO sign-off (and co-author) trailers in commit messages, with a GitHub noreply address only.
SIGNOFF_TRAILER = re.compile(r"(?:Signed-off-by|Co-authored-by): [^<>\n]+ <[\w.+-]+@users\.noreply\.github\.com>")
IPV4 = re.compile(r"(?<![\w.])(?:\d{1,3}\.){3}\d{1,3}(?!\w|\.\d)")
IPV6 = re.compile(r"(?<![\w:])(?:[0-9a-fA-F]{0,4}:){2,}[0-9a-fA-F:.]*(?![\w:])")
MAC = re.compile(r"(?<!\w)(?:[0-9a-f]{2}(?:[:-][0-9a-f]{2}){5}|"
                 r"[0-9a-f]{4}(?:\.[0-9a-f]{4}){2})(?![\w:])", re.IGNORECASE)
BARE_MAC = re.compile(r"(?<!\w)[0-9a-f]{12}(?!\w)", re.IGNORECASE)
MAC_KEYWORD = re.compile(r"\b(?:mac|hw|hwaddr|lladdr|ether)\b", re.IGNORECASE)
PRIVATE4 = tuple(ipaddress.ip_network(net) for net in (
    "10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16", "100.64.0.0/10"))
PRIVATE6 = ipaddress.ip_network("fc00::/7")
DOCUMENT6 = ipaddress.ip_network("2001:db8::/32")
GLOBAL6 = ipaddress.ip_network("2000::/3")


def mac_matches(text: str) -> Iterator[str]:
    for match in MAC.finditer(text):
        yield match.group()
    for match in BARE_MAC.finditer(text):
        # A keyword within 32 characters on the same line distinguishes bare MACs from hashes.
        before = text[max(0, match.start() - 32):match.start()].split("\n")[-1]
        after = text[match.end():match.end() + 32].split("\n")[0]
        if MAC_KEYWORD.search(before) or MAC_KEYWORD.search(after):
            yield match.group()


def matches(text: str) -> Iterator[tuple[str, str]]:
    """Yield rule and value; callers redact values before reporting."""
    for rule, pattern in RULES.items():
        for match in pattern.finditer(text):
            value = match.group()
            if rule == "email":
                domain = value.lower().split("@", 1)[1]
                if any(domain == example or domain.endswith("." + example)
                       for example in ("example.invalid", "example.com", "example.org", "example.net")):
                    continue
            yield rule, value
    for value in mac_matches(text):
        normal = re.sub(r"[:-]|\.", "", value.lower())
        if not normal.startswith("00005e0053") and normal not in ("0" * 12, "f" * 12):
            yield "mac", value
    for match in IPV4.finditer(text):
        try:
            address = ipaddress.IPv4Address(match.group())
        except ValueError:
            continue
        if any(address in network for network in PRIVATE4):
            yield "private-ipv4", match.group()
    for match in IPV6.finditer(text):
        value = match.group().rstrip(".")
        try:
            address = ipaddress.IPv6Address(value)
        except ValueError:
            continue
        mapped = address.ipv4_mapped
        if address.packed[11:13] == b"\xff\xfe":
            yield "mac", value  # Modified EUI-64 embeds a MAC in the low 64 bits.
        if mapped and any(mapped in network for network in PRIVATE4):
            yield "private-ipv4", value
        elif address in PRIVATE6:
            yield "private-ipv6", value
        elif address in GLOBAL6 and address not in DOCUMENT6:
            yield "global-ipv6", value
