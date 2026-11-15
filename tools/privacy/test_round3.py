"""Review round 3 regressions; all private-looking values are constructed fixtures."""

import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import git_input
import scan
from policy import load_literals
from test_round2 import commit_object

SEPARATORS = "\v\f\x1c\x1d\x1e\x85\u2028\u2029\r"
PRIVATE = "192" + ".168.77.240"


class LineTests(unittest.TestCase):
    def test_patch_retains_every_non_lf_separator(self) -> None:
        for separator in SEPARATORS:
            with self.subTest(separator=repr(separator)):
                text = "note" + separator + PRIVATE
                patch_text = "@@ -0,0 +1,2 @@\r\n+" + text + "\r\n+end\n"
                records = list(git_input.additions(patch_text.encode()))
                self.assertEqual(records, [(1, text), (2, "end")])
                self.assertEqual(list(scan.findings(
                    [("fixture", n, line, False) for n, line in records], set(), [])),
                    [("fixture", 1, "private-ipv4")])

    def test_new_blob_and_patch_use_the_same_line_numbers(self) -> None:
        for separator in SEPARATORS:
            with self.subTest(separator=repr(separator)):
                text = "note" + separator + "safe\r\n" + PRIVATE + "\n"
                with patch.object(git_input, "git", return_value=text.encode()):
                    records = list(git_input.blob_additions(Path("."), None, "blob"))
                patch_text = "@@ -0,0 +1,2 @@\n+" + text.replace("\n", "\n+").removesuffix("+")
                self.assertEqual(records, [(1, text.split("\n")[0].removesuffix("\r")), (2, PRIVATE)])
                self.assertEqual(records, list(git_input.additions(patch_text.encode())))

    def test_merge_missing_parent_file_does_not_hide_resolution_line(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            git_input.git(root, "init", "--quiet")
            base = commit_object(root, {})
            text = "note\fcontinued\n"
            first = commit_object(root, {"fixture": text}, (base,))
            second = commit_object(root, {"other": "safe\n"}, (base,))
            merged = commit_object(root, {"fixture": text + PRIVATE + "\n"}, (first, second))
            for parents in ([first, second], [second, first]):
                with self.subTest(parents=parents):
                    found = list(scan.findings(git_input.merge_changed(root, parents, merged), set(), []))
                    self.assertEqual(found, [("fixture", 2, "private-ipv4")])

    def test_message_cleanup_cannot_treat_embedded_hash_as_a_comment(self) -> None:
        for separator in SEPARATORS:
            with self.subTest(separator=repr(separator)):
                text = "Subject" + separator + "# " + PRIVATE + "\n"
                records = [("message", n, line, True) for n, line in scan.message_lines(text, True)]
                self.assertEqual(list(scan.findings(records, set(), [])), [("message", 1, "private-ipv4")])

    def test_local_literal_preserves_non_lf_separators(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "literals"
            for separator in SEPARATORS:
                with self.subTest(separator=repr(separator)):
                    literal = "synthetic" + separator + "#suffix"
                    path.write_bytes((literal + "\r\n").encode())
                    self.assertEqual(load_literals(path), [literal])

    def test_tree_and_recorded_messages_keep_git_line_numbers(self) -> None:
        text = "note\fcontinued\n" + PRIVATE + "\n"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "fixture").write_text(text)
            with patch.object(scan, "git", return_value=b"fixture\0"):
                found = list(scan.findings(scan.working_tree(root), set(), []))
            self.assertEqual(found, [("fixture", 2, "private-ipv4")])
        def fake_git(root: Path, *args: str) -> bytes:
            return text.encode() if args[0] == "show" else b"fork"
        with patch.object(git_input, "resolve", side_effect=lambda root, rev: rev), \
                patch.object(git_input, "git", side_effect=fake_git), \
                patch.object(git_input, "changed", return_value=iter([])):
            found = list(scan.findings(git_input.incoming(Path("."), "base", "head"), set(), []))
        self.assertEqual(found, [("commit:fork", 2, "private-ipv4")])


if __name__ == "__main__":
    unittest.main()
