"""Regression tests for review round 1, using invented values only."""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import git_input
import scan
from patterns import matches
from policy import digest

ROOT = Path(__file__).resolve().parents[2]


def rules(text: str) -> set[str]:
    return {rule for rule, _ in matches(text)}


class ScannerReviewTests(unittest.TestCase):
    def test_claude_code_sessions_are_detected(self) -> None:
        url = "https://claude.ai/" + "code/session_synthetic-id"
        self.assertIn("session-url", rules(url))

    def test_only_exact_commit_session_trailer_is_exempt(self) -> None:
        url = "https://claude.ai/" + "code/session_synthetic-id"
        trailer = "Claude-Session: " + url
        for name in ("commit-message", "commit:synthetic"):
            self.assertEqual(list(scan.findings([(name, 1, trailer, True)], set(), [])), [])
            for line in (url, " " + trailer, trailer + " ", trailer + " extra", trailer.lower(),
                         "Claude-Session: " + url.replace("https:", "http:"),
                         "Claude-Session: https://claude.ai/" + "chat/synthetic-id"):
                with self.subTest(name=name, line=line):
                    self.assertEqual(list(scan.findings([(name, 1, line, True)], set(), [])),
                                     [(name, 1, "session-url")])
        for name in ("fixture.txt", "commit-message", "commit:synthetic"):
            self.assertEqual(list(scan.findings([(name, 1, trailer, False)], set(), [])),
                             [(name, 1, "session-url")])
        self.assertEqual(list(scan.findings([("commit-message", 1, trailer, True)], set(), [url])),
                         [("commit-message", 1, "local-literal")])

    def test_private_ipv4_at_sentence_end(self) -> None:
        address = "10" + ".1.2.3"
        for suffix in (".", ". Next sentence", ",", ":22"):
            self.assertIn("private-ipv4", rules("root@" + address + suffix))
        for suffix in (".4", "4x", "x"):
            self.assertNotIn("private-ipv4", rules(address + suffix))

    def test_shared_ipv4_range_boundaries(self) -> None:
        for suffix in ("64.0.0", "127.255.255"):
            self.assertIn("private-ipv4", rules("100." + suffix))
        for suffix in ("63.255.255", "128.0.0"):
            self.assertNotIn("private-ipv4", rules("100." + suffix))

    def test_unique_local_ipv6_range(self) -> None:
        for prefix in ("fc00", "fd00", "fdff"):
            self.assertIn("private-ipv6", rules("[" + prefix + "::1234]"))
        for address in ("fbff::1234", "fe00::1234", "fe80::1", "2001:db8::1"):
            self.assertNotIn("private-ipv6", rules(address))

    def test_eui64_interface_identifier_embeds_mac(self) -> None:
        for prefix in ("fe80::", "2001:db8::", "fd00" + "::"):
            self.assertIn("mac", rules(prefix + "211:22ff:fe33:4455"))
        self.assertNotIn("mac", rules("fe80::211:22ff:fd33:4455"))
        self.assertNotIn("mac", rules("fe80:ff:fe::1"))

    def test_mac_after_colon_and_dotted(self) -> None:
        for address in ("c2" + ":8f:11:22:33:44", "c2" + "-8f-11-22-33-44", "c28f" + ".1122.3344"):
            self.assertIn("mac", rules("mac:" + address))
        for address in ("0000.5e00.5304", "0000.0000.0000", "ffff.ffff.ffff"):
            self.assertNotIn("mac", rules(address))

    def test_bare_mac_requires_nearby_keyword(self) -> None:
        address = "c28f" + "11223344"
        for line in ("mac:" + address, "HWADDR = " + address, "ether " + address, address + " (MAC)"):
            self.assertIn("mac", rules(line))
        for line in (address, "hash=" + address, "mac " + "x" * 80 + address,
                     "mac\n" + address, "mac 0" + address + "0", "mac " + "00005e005304"):
            self.assertNotIn("mac", rules(line))

    def test_exception_hashes_value_and_does_not_hide_another_match(self) -> None:
        first, second = "DHO" + "000000000001", "DHO" + "000000000002"
        allowed = {("fixture.txt", "serial", digest(first))}
        self.assertEqual(list(scan.findings([("fixture.txt", 1, "serial = " + first, False)], allowed, [])), [])
        for name, line in (("other.txt", first), ("fixture.txt", second), ("fixture.txt", first + " " + second)):
            self.assertEqual(list(scan.findings([(name, 1, line, False)], allowed, [])), [(name, 1, "serial")])


class HookReviewTests(unittest.TestCase):
    def test_cleanup_preserves_line_numbers_and_is_opt_in(self) -> None:
        text = "Subject\n# comment\n\nBody\n# ------------------------ >8 ------------------------\n+diff"
        self.assertEqual(list(scan.message_lines(text, True)), [(1, "Subject"), (3, ""), (4, "Body")])
        self.assertEqual(list(scan.message_lines(text, False)), list(enumerate(text.splitlines(), 1)))

    def test_hook_scans_recorded_text_without_comments_or_verbose_diff(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / ".githooks").mkdir()
            shutil.copyfile(ROOT / ".githooks/pre-commit", root / ".githooks/pre-commit")
            subprocess.run(["git", "init", "--quiet", str(root)], check=True, capture_output=True)
            subprocess.run(["git", "apply", str(ROOT / "tools/privacy/hooks.patch")],
                           cwd=root, check=True, capture_output=True)
            shutil.copytree(ROOT / "tools/privacy", root / "tools/privacy", ignore=shutil.ignore_patterns("__pycache__"))
            (root / "tools/privacy/allowlist.json").write_text("[]")
            message = root / "message with spaces.txt"
            private = "DHO" + "000000000001"
            suffix = "\n# " + private + "\n# ------------------------ >8 ------------------------\n+" + private
            message.write_text("Safe subject\n" + suffix)
            environment = {**os.environ, "PATH": str(Path(sys.executable).parent) + os.pathsep + os.environ["PATH"]}
            hook = [str(root / ".githooks/commit-msg"), str(message)]
            result = subprocess.run(hook, cwd=root, env=environment, capture_output=True, check=False)
            self.assertEqual(result.returncode, 0, result.stdout)
            self.assertEqual(message.read_text(), "Safe subject\n" + suffix)
            raw = subprocess.run([sys.executable, str(root / "tools/privacy/scan.py"), "--message", str(message)],
                                 cwd=root, env=environment, capture_output=True, check=False)
            self.assertEqual(raw.returncode, 1, raw.stdout)
            for body in (private, "#not-a-scissors-line\n" + private, " # " + private):
                message.write_text(body + suffix)
                result = subprocess.run(hook, cwd=root, env=environment, capture_output=True, check=False)
                self.assertEqual(result.returncode, 1, result.stdout)
                self.assertNotIn(private.encode(), result.stdout)


class RangeReviewTests(unittest.TestCase):
    def test_range_excludes_commits_reachable_from_upstream_refs(self) -> None:
        calls: list[tuple[str, ...]] = []
        def fake_git(root: Path, *args: str) -> bytes:
            calls.append(args)
            return b""
        with patch.object(git_input, "resolve", side_effect=lambda root, rev: rev), \
                patch.object(git_input, "git", side_effect=fake_git):
            list(git_input.incoming(Path("."), "base", "head", ["upstream/*"]))
        self.assertEqual(calls, [("rev-list", "--reverse", "base..head", "--not", "--remotes=upstream/*")])

    def test_recorded_messages_use_trailer_exception_but_scan_hash_comments(self) -> None:
        trailer = "Claude-Session: https://claude.ai/" + "code/session_synthetic-id"
        serial = "DHO" + "000000000001"
        def fake_git(root: Path, *args: str) -> bytes:
            if args[0] == "show":
                return (trailer + "\n# " + serial).encode()
            return b"fork"  # One incoming commit without parents.
        with patch.object(git_input, "resolve", side_effect=lambda root, rev: rev), \
                patch.object(git_input, "git", side_effect=fake_git), \
                patch.object(git_input, "changed", return_value=iter([])):
            found = list(scan.findings(git_input.incoming(Path("."), "base", "head"), set(), []))
        self.assertEqual(found, [("commit:fork", 2, "serial")])


if __name__ == "__main__":
    unittest.main()
