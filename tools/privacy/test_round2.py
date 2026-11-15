"""Review round 2 regressions with disposable Git objects and invented values."""

import contextlib
import io
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import git_input
import scan
from patterns import matches
from policy import load_allowlist

ROOT = Path(__file__).resolve().parents[2]
SCISSORS = " ------------------------ >8 ------------------------"


def object_id(root: Path, kind: str, data: bytes) -> str:
    result = subprocess.run(["git", "-C", str(root), "hash-object", "-w", "-t", kind, "--stdin"],
                            input=data, capture_output=True, check=True)
    return result.stdout.decode().strip()


def commit_object(root: Path, files: dict[str, str], parents: tuple[str, ...] = ()) -> str:
    # Raw fixture objects avoid commit hooks, user identity and changes to the real repository.
    tree = b"".join(b"100644 " + name.encode() + b"\0" + bytes.fromhex(object_id(root, "blob", text.encode()))
                    for name, text in sorted(files.items()))
    metadata = "tree " + object_id(root, "tree", tree) + "\n"
    metadata += "".join("parent " + parent + "\n" for parent in parents)
    metadata += "author Fixture <fixture@example.invalid> 1 +0000\n"
    metadata += "committer Fixture <fixture@example.invalid> 1 +0000\n\nSynthetic change\n"
    return object_id(root, "commit", metadata.encode())


class MergeTests(unittest.TestCase):
    def test_sync_merge_does_not_rescan_upstream_email(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            git_input.git(root, "init", "--quiet")
            base = commit_object(root, {"shared.txt": "Base\n"})
            fork = commit_object(root, {"shared.txt": "Base\n", "fork.txt": "Fork\n"}, (base,))
            files = {"shared.txt": "Base\n", "upstream.txt": "fixture@" + "mail.invalid\n"}
            upstream = commit_object(root, files, (base,))
            git_input.git(root, "update-ref", "refs/remotes/upstream/main", upstream)
            merge = commit_object(root, {**files, "fork.txt": "Fork\n"}, (fork, upstream))
            found = list(scan.findings(git_input.incoming(root, base, merge, ["upstream/*"]), set(), []))
            self.assertEqual(found, [])

    def test_conflict_resolution_only_scans_lines_added_against_every_parent(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            git_input.git(root, "init", "--quiet")
            base = commit_object(root, {"shared.txt": "Original\n"})
            fork = commit_object(root, {"shared.txt": "Fork\n"}, (base,))
            upstream_text = "Upstream\nfixture@" + "mail.invalid\n"
            upstream = commit_object(root, {"shared.txt": upstream_text}, (base,))
            git_input.git(root, "update-ref", "refs/remotes/upstream/main", upstream)
            text = upstream_text + "10" + ".23.45.67\n"
            merge = commit_object(root, {"shared.txt": text}, (fork, upstream))
            found = list(scan.findings(git_input.incoming(root, base, merge, ["upstream/*"]), set(), []))
            self.assertEqual(found, [("shared.txt", 3, "private-ipv4")])

    def test_octopus_merge_checks_all_parents(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            git_input.git(root, "init", "--quiet")
            base = commit_object(root, {"shared.txt": "Original\n"})
            first = commit_object(root, {"shared.txt": "First\n"}, (base,))
            second = commit_object(root, {"shared.txt": "Second\n"}, (base,))
            third_text = "Third\nfixture@" + "mail.invalid\n"
            third = commit_object(root, {"shared.txt": third_text}, (base,))
            merge = commit_object(root, {"shared.txt": third_text + "New\n"}, (first, second, third))
            found = list(scan.findings(git_input.incoming(root, third, merge), set(), []))
            self.assertEqual(found, [])


class MacTests(unittest.TestCase):
    def test_bare_mac_hardware_keywords(self) -> None:
        address = "c28f" + "11223344"
        for keyword in ("hw", "hwaddr", "lladdr", "HW", "LLADDR"):
            with self.subTest(keyword=keyword):
                self.assertIn("mac", {rule for rule, _ in matches(keyword + "=" + address)})
        for keyword in ("hardware", "somehw", "lladdress"):
            self.assertNotIn("mac", {rule for rule, _ in matches(keyword + "=" + address)})

    def test_mixed_mac_separators_and_reserved_examples(self) -> None:
        for suffix in (":8f-11:22-33:44", "-8f:11-22:33-44"):
            self.assertIn("mac", {rule for rule, _ in matches("c2" + suffix)})
        self.assertNotIn("mac", {rule for rule, _ in matches("00:00-5e:00-53:04")})


class MessageTests(unittest.TestCase):
    def run_message(self, setting: str | None, text: str, cleanup: bool = True) -> tuple[int, str]:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            git_input.git(root, "init", "--quiet")
            if setting is not None:
                git_input.git(root, "config", "core.commentChar", setting)
            message = root / "message.txt"
            message.write_text(text)
            policy = root / "policy.json"
            policy.write_text("[]")
            args = ["--message", str(message), "--allowlist", str(policy)]
            output = io.StringIO()
            with patch.object(scan.Path, "cwd", return_value=root), contextlib.redirect_stdout(output):
                result = scan.main(args + (["--git-message"] if cleanup else []))
            self.assertEqual(message.read_text(), text)
            return result, output.getvalue()

    def test_configured_comment_char_filters_comments_and_scissors(self) -> None:
        private = "10" + ".23.45.67"
        text = "Subject\n; " + private + "\n;" + SCISSORS + "\n+" + private
        self.assertEqual(self.run_message(";", text)[0], 0)
        self.assertEqual(self.run_message(";", text, cleanup=False)[0], 1)

    def test_configured_comment_char_does_not_hide_hash_body(self) -> None:
        result, output = self.run_message(";", "Subject\n# 10" + ".23.45.67\n; comment")
        self.assertEqual(result, 1)
        self.assertIn(":2: private-ipv4", output)

    def test_missing_config_uses_hash(self) -> None:
        self.assertEqual(self.run_message(None, "Subject\n# 10" + ".23.45.67")[0], 0)

    def test_auto_uses_scissors_marker_and_preserves_other_prefixes(self) -> None:
        private = "10" + ".23.45.67"
        suffix = "\n; " + private + "\n;" + SCISSORS + "\n+" + private
        self.assertEqual(self.run_message("auto", "Subject" + suffix)[0], 0)
        result, output = self.run_message("auto", "# " + private + suffix)
        self.assertEqual(result, 1)
        self.assertIn(":1: private-ipv4", output)

    def test_auto_without_scissors_retains_ambiguous_comments(self) -> None:
        self.assertEqual(self.run_message("auto", "Subject\n# 10" + ".23.45.67")[0], 1)

    def test_comment_config_read_failure_is_not_treated_as_absent(self) -> None:
        result = subprocess.CompletedProcess([], 2, b"", b"unreadable config")
        with patch.object(scan.subprocess, "run", return_value=result):
            with self.assertRaises(ValueError):
                scan.comment_char(Path("."), "Subject")


class ExceptionTests(unittest.TestCase):
    def test_entire_candidate_files_scan_clean(self) -> None:
        # Audit full contents, including unchanged lines missed by an additions-only scan.
        names = ["docs/development.md", "docs/fork/unit-dho804.md", "docs/fork/upstream-audit.md",
                 "docs/network.md", "docs/tasks/0024-privacy-checks.md", "tools/fw-sim.sh", "tools/ssh-sim.sh",
                 ".github/workflows/privacy.yml"]
        names += [str(path.relative_to(ROOT)) for path in (ROOT / "tools/privacy").rglob("*")
                  if path.is_file() and "__pycache__" not in path.parts]
        allowed = load_allowlist(ROOT / "tools/privacy/allowlist.json")
        for name in names:
            text = (ROOT / name).read_text()
            with self.subTest(name=name):
                self.assertEqual(list(scan.findings([(name, 1, text, False)], allowed, [])), [])


if __name__ == "__main__":
    unittest.main()
