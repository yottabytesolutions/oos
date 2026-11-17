"""Run the CI range shell against real disposable Git history, without fetch or commit commands."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

import git_input
from test_round2 import object_id

ROOT = Path(__file__).resolve().parents[2]
PRIVATE = "192" + ".168.77.240"


def snapshot(root: Path, files: dict[str, str], parents: tuple[str, ...] = ()) -> str:
    def tree_id(contents: dict[str, str]) -> str:
        directories: dict[str, dict[str, str]] = {}
        records = []
        for name, text in contents.items():
            if "/" in name:
                directory, remainder = name.split("/", 1)
                directories.setdefault(directory, {})[remainder] = text
            else:
                records.append((name, b"100644", object_id(root, "blob", text.encode())))
        records += [(name, b"40000", tree_id(children)) for name, children in directories.items()]
        data = b"".join(mode + b" " + name.encode() + b"\0" + bytes.fromhex(oid)
                        for name, mode, oid in sorted(records))
        return object_id(root, "tree", data)
    metadata = "tree " + tree_id(files) + "\n"
    metadata += "".join("parent " + parent + "\n" for parent in parents)
    metadata += "author Fixture <fixture@example.invalid> 1 +0000\n"
    metadata += "committer Fixture <fixture@example.invalid> 1 +0000\n\nSynthetic change\n"
    return object_id(root, "commit", metadata.encode())


class WorkflowObjectTests(unittest.TestCase):
    def run_range(self, fallback: bool, injected: bool = False, other_ref: bool = True,
                  pushed_ref: str = "topic") -> subprocess.CompletedProcess[str]:
        scanner = {"tools/privacy/" + name: (ROOT / "tools/privacy" / name).read_text()
                   for name in ("scan.py", "patterns.py", "policy.py", "git_input.py")}
        scanner["tools/privacy/allowlist.json"] = "[]"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            git_input.git(root, "init", "--quiet")
            base = snapshot(root, {} if fallback else scanner)
            if injected:
                adopted = snapshot(root, {"tools/privacy/scan.py": "raise SystemExit(0)\n"}, (base,))
            else:
                adopted = snapshot(root, {**scanner, "transient": PRIVATE + "\n"}, (base,)) if fallback else base
            # A PR can replace all scanner modules and remove an earlier private addition.
            head_files = ({"tools/privacy/scan.py": "raise SystemExit(0)\n"} if injected else
                          {name: "raise SystemExit(0)\n" for name in scanner if name.endswith(".py")})
            if not fallback or injected:
                head_files["fixture"] = PRIVATE + "\n"
            head = snapshot(root, head_files, (adopted,))
            git_input.git(root, "update-ref", "HEAD", head)
            git_input.git(root, "update-ref", "refs/remotes/origin/main", base)
            git_input.git(root, "update-ref", "refs/remotes/origin/" + pushed_ref, head)
            git_input.git(root, "symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/" + pushed_ref)
            if fallback and other_ref:
                git_input.git(root, "update-ref", "refs/remotes/origin/adopted", adopted)
            git_input.git(root, "read-tree", "--reset", "-u", "HEAD")
            workflow = (ROOT / ".github/workflows/privacy.yml").read_text().split("        run: |\n", 1)[1]
            script = "\n".join(line[10:] for line in workflow.splitlines() if line.startswith("          "))
            # Archive, range traversal and scanning are real; the workflow fetches nothing.
            environment = {**os.environ, "RUNNER_TEMP": directory, "PR_BASE": "" if fallback else base,
                           "PUSH_BASE": "0" * 40, "DEFAULT_BRANCH": "main",
                           "PUSH_REF": "refs/heads/" + pushed_ref}
            return subprocess.run(["bash", "-c", script], cwd=root, env=environment,
                                  text=True, capture_output=True, check=False)

    def test_pr_runs_base_modules_even_when_head_neuters_all_modules(self) -> None:
        result = self.run_range(False)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn('"fixture":1: private-ipv4 [redacted]', result.stdout)
        self.assertNotIn(PRIVATE, result.stdout + result.stderr)

    def test_new_branch_scans_transient_addition_using_oldest_adopted_scanner(self) -> None:
        result = self.run_range(True)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn('"transient":1: private-ipv4 [redacted]', result.stdout)
        self.assertNotIn("::notice::", result.stdout)
        self.assertNotIn(PRIVATE, result.stdout + result.stderr)

    def test_two_commit_push_cannot_introduce_its_own_always_pass_scanner(self) -> None:
        for pushed_ref in ("topic", "feature/privacy"):
            with self.subTest(pushed_ref=pushed_ref):
                result = self.run_range(True, injected=True, other_ref=False, pushed_ref=pushed_ref)
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn("::error::", result.stdout)
                self.assertNotIn("::notice::", result.stdout)
                self.assertNotIn(PRIVATE, result.stdout + result.stderr)

    def test_fallback_rejects_real_scanner_reachable_only_from_pushed_ref(self) -> None:
        result = self.run_range(True, other_ref=False)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("::error::", result.stdout)
        self.assertNotIn('"transient":1:', result.stdout)


if __name__ == "__main__":
    unittest.main()
