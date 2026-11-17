# SPDX-License-Identifier: GPL-2.0-or-later
"""Synthetic scanner regressions. No network, hardware or Git commit is needed."""

import contextlib
import io
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import git_input
import scan
from patterns import matches
from policy import digest, load_allowlist, load_literals


def rules(text: str) -> set[str]:
    return {name for name, _ in matches(text)}


class PatternTests(unittest.TestCase):
    def test_private_ipv4_boundaries(self) -> None:
        for address in ("10" + ".42.0.1", "172" + ".16.0.1", "172" + ".31.255.255", "192" + ".168.4.2"):
            self.assertIn("private-ipv4", rules(address))
        for address in ("192.0.2.5", "198.51.100.4", "203.0.113.1", "127.0.0.1", "169.254.2.3",
                        "172.15.0.1", "172.32.0.1", "999.0.0.1", "v10.2.3.4.5"):
            self.assertNotIn("private-ipv4", rules(address))

    def test_ipv6_documentation_and_special_ranges(self) -> None:
        for address in ("2001:db8::5", "2001:DB8:1::9", "::1", "fe80::1", "fd00" + "::/8", "::"):
            self.assertNotIn("global-ipv6", rules(address))
        self.assertIn("global-ipv6", rules("[" + "2001:4860" + "::5]:8080"))
        self.assertIn("private-ipv4", rules("::ffff:" + "10" + ".42.0.1"))

    def test_mac_and_placeholders(self) -> None:
        self.assertIn("mac", rules("02" + ":11:22:33:44:55"))
        self.assertIn("mac", rules("02" + "-11-22-33-44-55"))
        for address in ("00:00:5e:00:53:05", "00:00:00:00:00:00", "ff:ff:ff:ff:ff:ff"):
            self.assertNotIn("mac", rules(address))

    def test_home_paths_and_required_device_paths(self) -> None:
        for text in ("/" + "Users/synthetic/work", "/" + "home/synthetic/work", "C:" + "\\Users\\synthetic\\work"):
            self.assertIn("home-path", rules(text))
        self.assertEqual(rules("/path/to/key /dev/xdma0 /sys/class/net /oos/system.conf"), set())

    def test_email_serial_key_token_session_and_host(self) -> None:
        examples = {
            "email": "synthetic@" + "mail.invalid",
            "serial": "DHO" + "000000000001",
            "private-key": "-----BEGIN " + "OPENSSH PRIVATE KEY-----",
            "token": "ghp_" + "A" * 36,
            "session-url": "https://chatgpt.com/" + "c/synthetic-session",
        }
        for rule, value in examples.items():
            with self.subTest(rule=rule):
                self.assertIn(rule, rules(value))
        self.assertEqual(rules("scope@example.invalid https://example.invalid /path/to/ <scope-address>"), set())
        self.assertEqual(rules("scope@sub.example.invalid DspGenerator"), set())
        for prefix in ("ghp_", "github_pat_", "sk-proj-", "glpat-", "xoxb-"):
            self.assertIn("token", rules(prefix + "A" * 40))
        self.assertIn("session-url", rules("https://chatgpt.com/" + "codex/tasks/synthetic-task"))


class PolicyTests(unittest.TestCase):
    def test_commit_messages_cannot_use_fixture_exceptions(self) -> None:
        text = "DHO" + "000000000001"
        for name in ("commit-message", "commit:synthetic"):
            allowed = {(name, "serial", digest(text))}
            self.assertEqual(list(scan.findings([(name, 1, text, True)], allowed, [])), [(name, 1, "serial")])

    def test_repository_exceptions_are_exact_and_cover_synthetic_fixtures(self) -> None:
        root = Path(__file__).resolve().parents[2]
        allowed = load_allowlist(root / "tools/privacy/allowlist.json")
        for name, rule, fingerprint in allowed:
            text = (root / name).read_text(encoding="utf-8", errors="replace")
            self.assertIn((rule, fingerprint), {(r, digest(v)) for r, v in matches(text)}, name)
        fixture = root / "tools/privacy/fixtures/synthetic.txt"
        text = fixture.read_text(encoding="utf-8")
        self.assertEqual(list(scan.findings([(fixture.relative_to(root).as_posix(), 1, text, False)], allowed, [])), [])

    def test_exact_value_exception_and_literal_priority(self) -> None:
        text = "DHO" + "000000000001"
        record = ("fixture.txt", 1, text, False)
        allowed = {("fixture.txt", "serial", digest(text))}
        self.assertEqual(list(scan.findings([record], allowed, [])), [])
        self.assertEqual(list(scan.findings([record], allowed, [text])), [("fixture.txt", 1, "local-literal")])
        for altered in (("other.txt", 1, text, False), ("fixture.txt", 1, text + "2", False)):
            self.assertEqual(len(list(scan.findings([altered], allowed, []))), 1)

    def test_allowlist_schema_and_duplicates(self) -> None:
        entry = {"path": "fixture.txt", "rule": "serial", "sha256": digest("synthetic"), "reason": "Synthetic fixture"}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "allowlist.json"
            path.write_text(json.dumps([entry]), encoding="utf-8")
            self.assertEqual(len(load_allowlist(path)), 1)
            for bad in ([entry, entry], [{**entry, "path": "*.txt"}], [{**entry, "reason": ""}],
                        [{**entry, "rule": "typo"}], [{**entry, "sha256": "bad"}],
                        [{**entry, "path": "../fixture.txt"}], {"not": "an array"}):
                path.write_text(json.dumps(bad), encoding="utf-8")
                with self.assertRaises(ValueError):
                    load_allowlist(path)

    def test_local_literal_format(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "literals.txt"
            self.assertEqual(load_literals(path), [])
            path.write_text("# comment\n\n synthetic exact \n", encoding="utf-8")
            self.assertEqual(load_literals(path), [" synthetic exact "])


class GitInputTests(unittest.TestCase):
    def test_added_lines_exclude_context_deletions_and_headers(self) -> None:
        diff = b"--- a/f\n+++ b/f\n@@ -2,2 +2,2 @@\n-deleted\n+added\n context\n@@ -8 +9 @@\n+another\n"
        self.assertEqual(list(git_input.additions(diff)), [(2, "added"), (9, "another")])
        self.assertEqual(list(git_input.additions(b"@@ -0,0 +1 @@\n+++legitimate content\n")), [(1, "++legitimate content")])

    def test_changed_blobs_force_text_and_disable_external_filters(self) -> None:
        def fake_git(root: Path, *args: str) -> bytes:
            if args[0] == "cat-file":
                return b"old\nnew\0text\n"
            self.assertIn("--text", args)
            self.assertIn("--no-ext-diff", args)
            self.assertIn("--no-textconv", args)
            return b"@@ -1 +2 @@\n+new\0text\n"
        with patch.object(git_input, "entries", return_value=iter([("odd\nname.txt", "newoid")])), \
                patch.object(git_input, "git", side_effect=fake_git):
            records = list(git_input.changed(Path("."), {"odd\nname.txt": "oldoid"}, None))
        self.assertEqual(records[1][1:3], (2, "new\0text"))

    def test_index_and_tree_nul_paths_and_submodules(self) -> None:
        for revision, raw in ((None, b"100644 abc 0\todd\nname\0"),
                              ("revision", b"100644 blob abc\todd\nname\0")):
            with patch.object(git_input, "git", return_value=raw):
                self.assertEqual(list(git_input.entries(Path("."), revision)), [("odd\nname", "abc")])
        with patch.object(git_input, "git", return_value=b"160000 commit abc\tsub\0"):
            self.assertEqual(list(git_input.entries(Path("."), "revision")), [])
        with patch.object(git_input, "git", return_value=b"100644 abc 2\tconflict\0"):
            with self.assertRaises(ValueError):
                list(git_input.entries(Path("."), None))

    def test_range_scans_each_commit_including_transient_leak_and_messages(self) -> None:
        seen: list[str] = []
        def fake_git(root: Path, *args: str) -> bytes:
            if args[0] == "rev-list" and "--reverse" in args:
                return b"first\nsecond\n"
            if args[0] == "show":
                return ("DHO" + "000000000001").encode()
            return (args[-1] + " parent").encode()
        def fake_changed(root: Path, old: dict[str, str], revision: str | None) -> list[scan.Record]:
            assert revision is not None
            seen.append(revision)
            return [("fixture.txt", 1, "DHO" + "000000000001", False)] if revision == "first" else []
        with patch.object(git_input, "resolve", side_effect=lambda root, rev: rev), \
                patch.object(git_input, "git", side_effect=fake_git), \
                patch.object(git_input, "entries", return_value=iter([])), \
                patch.object(git_input, "changed", side_effect=fake_changed):
            found = list(scan.findings(git_input.incoming(Path("."), "base", "head"), set(), []))
        self.assertEqual(seen, ["first", "second"])
        self.assertEqual(len(found), 3)

    def test_staged_index_in_real_repository_without_commits(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for args in (("init", "--quiet"),):
                # This isolated, disposable repository needs no commits.
                result = subprocess.run(["git", "-C", str(root), *args], capture_output=True, check=False)
                self.assertEqual(result.returncode, 0)
            file = root / "odd\nfixture.txt"
            private = "DHO" + "000000000001"
            file.write_text(private, encoding="utf-8")
            git_input.git(root, "add", "--", file.name)
            file.write_text("unstaged safe replacement", encoding="utf-8")
            found = list(scan.findings(git_input.staged(root), set(), []))
            self.assertEqual(found, [(file.name, 1, "serial")])

    def test_git_failure_is_closed_and_redacted(self) -> None:
        result = subprocess.CompletedProcess([], 1, b"", b"synthetic private detail")
        with patch.object(git_input.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(ValueError, "Git input failed"):
                git_input.git(Path("."), "show")


class CliTests(unittest.TestCase):
    def test_names_redact_values_and_escape_terminal_control_characters(self) -> None:
        private = "DHO" + "000000000001"
        name = scan.safe_name(private + "\n\x1b[31m-private-marker", ["private-marker"])
        self.assertNotIn(private, name)
        self.assertNotIn("private-marker", name)
        self.assertNotIn("\x1b", name)
        self.assertIn("<redacted>", name)

    def test_message_output_is_redacted_and_exit_codes_distinguish_errors(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "tools/privacy").mkdir(parents=True)
            (root / "tools/privacy/allowlist.json").write_text("[]", encoding="utf-8")
            message = root / "message.txt"
            private = "DHO" + "000000000001"
            message.write_text(private, encoding="utf-8")
            def fake_git(path: Path, *args: str) -> bytes:
                return str(root).encode() if args[0] == "rev-parse" else b""
            output = io.StringIO()
            with patch.object(scan, "git", side_effect=fake_git), contextlib.redirect_stdout(output):
                self.assertEqual(scan.main(["--message", str(message)]), 1)
            self.assertNotIn(private, output.getvalue())
            self.assertIn("serial [redacted]", output.getvalue())
            message.write_text("Safe message", encoding="utf-8")
            with patch.object(scan, "git", side_effect=fake_git), contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(scan.main(["--message", str(message)]), 0)
            with patch.object(scan, "git", side_effect=ValueError("private")), \
                    contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(scan.main(["--tree"]), 2)


class HookTests(unittest.TestCase):
    def test_prepared_patch_preserves_gate_and_passes_message_path(self) -> None:
        root = Path(__file__).resolve().parents[2]
        patch_file = root / "tools/privacy/hooks.patch"
        with tempfile.TemporaryDirectory() as directory:
            scratch = Path(directory)
            (scratch / ".githooks").mkdir()
            original = (root / ".githooks/pre-commit").read_text()
            (scratch / ".githooks/pre-commit").write_text(original)
            subprocess.run(["git", "apply", str(patch_file)], cwd=scratch, check=True, capture_output=True)
            pre_commit = scratch / ".githooks/pre-commit"
            self.assertIn("exec cargo xtask check --fast", pre_commit.read_text())
            (scratch / "tools/privacy").mkdir(parents=True)
            (scratch / "tools/privacy/scan.py").write_text(
                "import pathlib, sys\n"
                "pathlib.Path('args.txt').write_text(repr(sys.argv[1:]))\n"
                "sys.exit(1 if pathlib.Path('refuse').exists() else 0)\n"
            )
            bin_dir = scratch / "bin"
            bin_dir.mkdir()
            cargo = bin_dir / "cargo"
            cargo.write_text("#!/bin/sh\nprintf '%s' \"$*\" > gate.txt\nexit 7\n")
            cargo.chmod(0o755)
            environment = {**os.environ, "PATH": str(bin_dir) + os.pathsep + os.environ["PATH"]}
            result = subprocess.run(["sh", str(pre_commit)], cwd=scratch, env=environment, check=False)
            self.assertEqual(result.returncode, 7)
            self.assertEqual((scratch / "gate.txt").read_text(), "xtask check --fast")
            (scratch / "gate.txt").unlink()
            (scratch / "refuse").touch()
            result = subprocess.run(["sh", str(pre_commit)], cwd=scratch, env=environment, check=False)
            self.assertEqual(result.returncode, 1)
            self.assertFalse((scratch / "gate.txt").exists())
            hook = scratch / ".githooks/commit-msg"
            result = subprocess.run([str(hook), "message with spaces.txt"], cwd=scratch, check=False)
            self.assertEqual(result.returncode, 1)
            self.assertEqual((scratch / "args.txt").read_text(),
                             repr(["--message", "message with spaces.txt", "--git-message"]))


if __name__ == "__main__":
    unittest.main()
