"""Execute the CI shell with fake Git and scanner commands, without network or commits."""

import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FAKES = r'''
git() {
  printf '<%s>' "$@" >> "$RUNNER_TEMP/git-calls"
  printf '\n' >> "$RUNNER_TEMP/git-calls"
  case "$1" in
    fetch) return "$FETCH_EXIT" ;;
    cat-file)
      case "$3" in
        oldest:tools/privacy/scan.py) return "$HISTORY_ADOPTION_EXIT" ;;
        *:tools/privacy/scan.py) return "$ADOPTION_EXIT" ;;
        *:tools/privacy/allowlist.json) return "$POLICY_EXIT" ;;
        fallback^{commit}) return 0 ;;
        *) return "$BEFORE_EXIT" ;;
      esac ;;
    merge-base)
      if [ "$2" = --is-ancestor ]; then return "$ANCESTOR_EXIT"; fi
      printf 'fallback\n' ;;
    show) printf '[{"trusted-base":true}]\n' ;;
    rev-list)
      if [ "$2" = HEAD ]; then
        printf '%s' "$PUSHED_ONLY"
        return "$REACHABILITY_EXIT"
      fi
      printf '%s' "$RANGE_COMMITS" ;;
    archive)
      if [ "$ARCHIVE_EXIT" != 0 ]; then return "$ARCHIVE_EXIT"; fi
      tar -cf - -C "$RUNNER_TEMP/trusted" tools/privacy ;;
    *) return 99 ;;
  esac
}
'''
TRUSTED_SCANNER = '''import os, sys
from pathlib import Path
root = Path(os.environ["RUNNER_TEMP"])
(root / "scan-args").write_text("\\n".join([__file__, *sys.argv[1:]]))
(root / "used-policy").write_bytes(Path(sys.argv[sys.argv.index("--allowlist") + 1]).read_bytes())
sys.exit(int(os.environ["SCAN_EXIT"]))
'''


class WorkflowTests(unittest.TestCase):
    def run_workflow(self, **overrides: str) -> tuple[subprocess.CompletedProcess[str], str, list[str], str]:
        script = (ROOT / ".github/workflows/privacy.yml").read_text().split("        run: |\n", 1)[1]
        script = "\n".join(line[10:] for line in script.splitlines() if line.startswith("          "))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "tools/privacy").mkdir(parents=True)
            (root / "tools/privacy/allowlist.json").write_text('[{"untrusted-head":true}]')
            (root / "tools/privacy/scan.py").write_text("raise SystemExit(0)\n")
            trusted = root / "trusted/tools/privacy"
            trusted.mkdir(parents=True)
            (trusted / "scan.py").write_text(TRUSTED_SCANNER)
            environment = {**os.environ, "RUNNER_TEMP": directory, "PR_BASE": "", "PUSH_BASE": "before",
                           "DEFAULT_BRANCH": "main", "BEFORE_EXIT": "0", "ANCESTOR_EXIT": "0",
                           "POLICY_EXIT": "0", "ADOPTION_EXIT": "0", "FETCH_EXIT": "0", "SCAN_EXIT": "0",
                           "ARCHIVE_EXIT": "0", "HISTORY_ADOPTION_EXIT": "0", "RANGE_COMMITS": "",
                           "PUSH_REF": "refs/heads/topic", "PUSHED_ONLY": "head\n",
                           "REACHABILITY_EXIT": "0", **overrides}
            result = subprocess.run(["bash", "-c", FAKES + script], cwd=root, env=environment,
                                    text=True, capture_output=True, check=False)
            calls = (root / "git-calls").read_text()
            args = (root / "scan-args").read_text().splitlines() if (root / "scan-args").exists() else []
            policy = (root / "used-policy").read_text() if (root / "used-policy").exists() else ""
            return result, calls, args, policy

    def test_missing_zero_and_force_push_before_fall_back_to_merge_base(self) -> None:
        for override in ({"PUSH_BASE": ""}, {"PUSH_BASE": "0" * 40}, {"BEFORE_EXIT": "1"},
                         {"ANCESTOR_EXIT": "1"}):
            with self.subTest(override=override):
                result, calls, args, _ = self.run_workflow(**override)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("<merge-base><origin/main><HEAD>", calls)
                self.assertEqual(args[args.index("--range") + 1:args.index("--range") + 3], ["fallback", "HEAD"])

    def test_push_and_pull_request_use_available_base(self) -> None:
        for override, base in (({}, "before"), ({"PR_BASE": "pr-base", "ANCESTOR_EXIT": "1"}, "pr-base")):
            result, calls, args, policy = self.run_workflow(**override)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertNotIn("<merge-base><origin/main><HEAD>", calls)
            self.assertIn("<show><" + base + ":tools/privacy/allowlist.json>", calls)
            self.assertEqual(args[args.index("--range") + 1], base)
            self.assertEqual(policy.strip(), '[{"trusted-base":true}]')

    def test_absent_base_policy_uses_empty_policy(self) -> None:
        result, calls, args, policy = self.run_workflow(POLICY_EXIT="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("<cat-file><-e><before:tools/privacy/scan.py>", calls)
        self.assertIn("--range", args)
        self.assertEqual(policy.strip(), "[]")

    def test_pre_adoption_base_prints_notice_and_skips_scan(self) -> None:
        result, calls, args, policy = self.run_workflow(ADOPTION_EXIT="1")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("::notice::", result.stdout)
        self.assertIn("not adopted", result.stdout)
        self.assertEqual(args, [])
        self.assertEqual(policy, "")
        self.assertNotIn("<show>", calls)

    def test_unavailable_pull_request_base_is_not_treated_as_pre_adoption(self) -> None:
        result, _, args, _ = self.run_workflow(PR_BASE="missing-base", BEFORE_EXIT="1", ADOPTION_EXIT="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertNotIn("::notice::", result.stdout)
        self.assertEqual(args, [])

    def test_fetch_upstream_and_exclude_all_upstream_branches(self) -> None:
        result, calls, args, _ = self.run_workflow()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("<fetch><--no-tags><https://github.com/deckerjulian/oos.git>"
                      "<+refs/heads/*:refs/remotes/upstream/*>", calls)
        self.assertEqual(args[-2:], ["--exclude-remotes", "upstream/*"])

    def test_fetch_failure_fails_closed(self) -> None:
        for adopted in ("0", "1"):
            result, _, args, _ = self.run_workflow(FETCH_EXIT="1", ADOPTION_EXIT=adopted)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(args, [])

    def test_head_cannot_neuter_exported_scanner(self) -> None:
        result, calls, args, _ = self.run_workflow(PR_BASE="pr-base", SCAN_EXIT="1")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("<archive><pr-base><tools/privacy>", calls)
        self.assertTrue(Path(args[0]).is_absolute())
        self.assertNotEqual(args[0], "tools/privacy/scan.py")

    def test_export_failure_fails_closed(self) -> None:
        result, _, args, _ = self.run_workflow(ARCHIVE_EXIT="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(args, [])

    def test_head_tests_cannot_gate_range_scan(self) -> None:
        workflow = (ROOT / ".github/workflows/privacy.yml").read_text()
        self.assertLess(workflow.index("name: Incoming additions"), workflow.index("name: Scanner regression tests"))

    def test_fallback_uses_oldest_adopted_range_scanner_even_if_head_removed_it(self) -> None:
        for override in ({"PUSH_BASE": ""}, {"PUSH_BASE": "0" * 40}, {"BEFORE_EXIT": "1"},
                         {"ANCESTOR_EXIT": "1"}):
            with self.subTest(override=override):
                result, calls, args, policy = self.run_workflow(
                    **override, ADOPTION_EXIT="1", RANGE_COMMITS="preadoption\noldest\nhead\n", POLICY_EXIT="1")
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("<archive><oldest><tools/privacy>", calls)
                self.assertNotIn("<archive><head>", calls)
                self.assertEqual(args[args.index("--range") + 1:args.index("--range") + 3], ["fallback", "HEAD"])
                self.assertEqual(policy.strip(), "[]")
                self.assertIn("<rev-list><--reverse><--topo-order><fallback..HEAD>"
                              "<--not><--remotes=upstream/*>", calls)
                self.assertIn("<rev-list><HEAD><--not><--exclude=origin/topic>"
                              "<--exclude=origin/HEAD><--remotes=origin>", calls)

    def test_fallback_cannot_skip_without_trusted_range_scanner(self) -> None:
        result, _, args, _ = self.run_workflow(PUSH_BASE="", ADOPTION_EXIT="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(args, [])
        self.assertIn("::error::", result.stdout)

    def test_fallback_fails_closed_for_a_branch_that_never_adopted_scanner(self) -> None:
        result, _, args, _ = self.run_workflow(PUSH_BASE="", ADOPTION_EXIT="1", RANGE_COMMITS="preadoption\n")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn("::error::", result.stdout)
        self.assertEqual(args, [])

    def test_fallback_fails_closed_if_origin_reachability_query_fails(self) -> None:
        result, _, args, _ = self.run_workflow(PUSH_BASE="", ADOPTION_EXIT="1", REACHABILITY_EXIT="1")
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(args, [])

    def test_fallback_rejects_oldest_scanner_when_only_reachable_from_pushed_ref(self) -> None:
        result, calls, args, _ = self.run_workflow(
            PUSH_BASE="", ADOPTION_EXIT="1", RANGE_COMMITS="oldest\nhead\n", PUSHED_ONLY="oldest\nhead\n")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertNotIn("<archive>", calls)
        self.assertEqual(args, [])


if __name__ == "__main__":
    unittest.main()
