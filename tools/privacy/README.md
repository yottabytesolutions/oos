# Privacy scanner

Python 3.10 or newer, standard library only. Run from the repository root:

```sh
python3 tools/privacy/scan.py --staged
python3 tools/privacy/scan.py --message target/COMMIT_MSG.txt
python3 tools/privacy/scan.py --range main HEAD
python3 tools/privacy/scan.py --tree
python3 -m unittest discover -s tools/privacy -p 'test_*.py' -v
```

Exit codes: 0 means no findings, 1 means findings, 2 means an input or policy error. Findings show the repository
path, new line number and rule name, with values redacted. Line 0 means the filename itself. Outputs quote unusual
filenames to prevent terminal escapes and CI command injection. Git failures fail closed. No command writes Git
metadata, accesses hardware or uses the network.

`--staged` reads the actual index blobs and scans added lines against HEAD, including new files, renames and symlink
targets. An unborn branch scans all index content. It ignores unstaged replacements. Deleted and unchanged text
does not create findings. Blob diffs force text and disable external diff and text-conversion commands; binary
attributes cannot hide textual secrets. Submodule contents are separate repositories and are not inspected.

`--range BASE HEAD` scans every commit reachable from HEAD but not BASE, including side commits. It checks each
message and compares each non-merge tree with its parent, so a value added and then removed still fails. For merges,
only result lines added against every parent are scanned, equivalent to combined-diff additions. Text inherited
from any parent is excluded; new conflict-resolution text is checked. Both revisions must exist locally.
Add `--exclude-remotes 'upstream/*'` to omit commits
reachable from upstream remote refs, equivalent to `git rev-list BASE..HEAD --not --remotes='upstream/*'`.
Exclusion is by commit reachability: a fork commit that copies upstream text still gets scanned.
`--tree` audits tracked and nonignored new content,
including symlink targets; it excludes ignored local data and history. Stage new files before
relying on the staged check. No mode rewrites history or prints matched values.

The generic rules cover colon, hyphen (including mixed separators) and dotted MAC addresses, plus bare 12-hex MACs
within 32 characters of a `mac`, `hw`, `hwaddr`, `lladdr` or `ether` keyword on the same line.
They cover RFC1918 and shared-space IPv4 (`100.64.0.0/10`,
also IPv4-mapped IPv6), unique-local IPv6 (`fc00::/7`), EUI-64 interface identifiers embedding a MAC, global IPv6 outside the documentation
range, absolute Unix and Windows home paths, Rigol serial-like identifiers, email addresses, common private-key
headers and token formats, and ChatGPT/Claude session links (including Claude code sessions). Documentation IP ranges,
loopback, link-local addresses without embedded MACs, reserved example MACs, example email domains and required
device paths are supported. A generic scanner
cannot recognize arbitrary secrets, encoded data or every personal hostname: add exact local literals and review
the diff. Very broad network prefixes can trigger the global IPv6 rule and need an explained exception.

Session URLs are flagged in every source. Only commit messages exempt an exact line of the form
`Claude-Session: https://claude.ai/code/session_<id>`; the id contains letters, digits, underscores or hyphens.
There must be no leading/trailing whitespace or other text. The owner's publish step strips that trailer.
The exemption never applies to file contents or filenames, other session URLs or local private literals.

## Local private literals

Create `local/privacy-literals.txt` locally. It is ignored by Git. Put one exact, case-sensitive private value on
each line, for example a bench hostname or unit identifier. Blank lines and lines beginning with `#` are ignored;
other leading and trailing spaces are significant. Never copy these values into tests, a task file, an exception
reason or a commit message. A tracked literal file is an error. Local literals always fail even if the file has
a generic-pattern exception. Commit messages cannot use fixture exceptions. CI has only generic patterns; the local
file is never uploaded.

## Reviewed synthetic exceptions

`allowlist.json` is a JSON array. Each entry has exactly four string fields:

```json
{
  "path": "tools/privacy/fixtures/synthetic.txt",
  "rule": "serial",
  "sha256": "<SHA-256 of the exact matched value encoded as UTF-8>",
  "reason": "Invented zero-filled identifier used only to test the scanner."
}
```

The key is `(path, rule, sha256(matched_value))`. Paths and rules are exact, without globs. The hash covers only
the matched value, including its exact case and separators. Unrelated file edits do not invalidate an exception;
a distinct matching value in the same file still fails, even on the same line. Review the context of every use
of an exempt value. A header-only rule such as `private-key` cannot distinguish keys sharing that header. No exception
is needed for the synthetic calibration containers under `hal/testdata/synthetic/`: their data contain no matching
text. The signing fixture in `keys/community-signing.key` is intentionally public (see `docs/update.md`); any new
matching key fixture still requires its own reviewed exception. Do not blanket-allow a directory.

To add an invented fixture:

1. Write the fixture using only invented values. Run the scanner to identify its rule names.
2. Compute the matching hashes without printing values. This example computes `serial` hashes:

   ```sh
   python3 - <<'PY'
   import sys
   from pathlib import Path
   sys.path.insert(0, 'tools/privacy')
   from patterns import matches
   from policy import digest
   from git_input import text_lines
   path = Path('tools/privacy/fixtures/synthetic.txt')
   hashes = {digest(value) for line in text_lines(path.read_bytes().decode('utf-8', errors='replace'))
             for rule, value in matches(line) if rule == 'serial'}
   for fingerprint in sorted(hashes):
       print(fingerprint)
   PY
   ```

3. Add one policy entry per path, rule and matched-value hash, with a reason explaining why it is synthetic.
   Add a regression proving that another value still fails, and run the Python suite.
4. Obtain independent review and land the policy before the fixture that needs it on an adopted base.
   Initial adoption may introduce the reviewed scanner, policy, job and fixtures together on a normal base.
   A push using a fallback base requires adoption already reachable from another origin ref, as described below,
   and may scan the adoption commit itself.

The radio simulator retains its invented private subnets: the existing ignored UI `live_radio` test pins that
network prefix. Its exceptions cover only those matched values in the exact simulator file. Changing both sides of that test contract
requires a separate reservation for the UI source. The firewall simulator and network documentation also have
exact exceptions for the deliberately rejected broad IPv6 allocation prefix, which is not a unit address.
The firewall simulator's invented unique-local prefix and member address also have exact exceptions: they test
on-link ULA handling without depending on a real network.

CI treats `tools/privacy/scan.py` in the base revision as the adoption marker. Once adopted, CI exports the entire
`tools/privacy/` directory from that revision into the runner's temporary directory and runs that copy against the
checked-out incoming range. With the reviewed workflow intact, scanner edits in the head cannot disable this scan.
CI uses the range base's allowlist,
or an empty array if that revision has no policy. It never trusts the head's own exceptions. Head regression tests
run afterward in a separate step, even when the range scan fails; their result cannot prevent the range scan.

When a valid ordinary push or pull-request base has no scanner, the job prints a notice and exits 0 without
scanning the range. For a push whose before SHA is missing, zero, unavailable or not an ancestor, the default-branch
merge base is the range base. If that base predates adoption, CI selects the oldest non-upstream commit in the
incoming range that contains the scanner and is also reachable from another origin remote ref. In reverse
topological order, it rejects every commit listed by
`git rev-list HEAD --not --exclude="origin/${PUSH_REF#refs/heads/}" --exclude=origin/HEAD --remotes=origin`.
The exclusions precede `--remotes` because they apply to that next ref expansion
([Git revision options](https://git-scm.com/docs/git-rev-list#Documentation/git-rev-list.txt---excludeltglob-patterngt)).
The pushed branch and origin's symbolic HEAD alias cannot establish scanner trust. Thus a push that first adds an
always-pass scanner and then adds private data cannot select its own scanner. CI exports the selected scanner and
scans the whole range, including adoption and transient additions, using the range base's policy (usually empty).
This still applies if a later commit deletes or disables the scanner. If no eligible scanner exists, CI fails
closed, including for a fallback branch that never adopted the scanner. An unavailable pull-request base, failed
upstream fetch, failed reachability query or failed export also fails the job.

On `pull_request`, GitHub runs the workflow with head changes included in the PR merge context, so the head can
change the workflow file itself ([GitHub event documentation](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows#pull_request)).
Exporting a base scanner does not protect against a head workflow that removes or replaces the scan step.
Base-scanner trust therefore relies on branch protection requiring the Privacy check and CODEOWNERS review of
`.github/` and `tools/privacy/`, including changes to the ownership rules themselves. These paths are mapped in
`.github/CODEOWNERS` using the repository's commented placeholder format. The owner must enable real teams and
required code-owner approvals in repository settings; commented entries do not enforce review. Those settings
have not been verified or configured by this task. Changes to the scanner and workflow also require the same
independent review as other code. Existing findings outside incoming additions are reported by the tree audit,
not silently allowlisted.

## Scanner CLI changes

The workflow from the head supplies flags to the scanner exported from the base. Roll out a new CLI flag in two
separate changes:

1. Add and test support for the flag in the scanner while leaving workflow invocations compatible with the old
   scanner. Review and land this change first on every target base that will run the workflow.
2. Once those bases support the flag, change the workflow invocation to pass it in a later reviewed change.

For fallback pushes, the selected scanner from another origin ref must also support the flag before step 2.
Introducing the flag and its workflow use together would run an older base scanner with an unknown argument and
fail the job. Keep existing flags compatible while workflows still pass them; remove workflow use before removing
scanner support.

## Hook and gate integration

The current repository rules forbid editing `.githooks/` or changing `core.hooksPath`. Hook installation is pending
the coordinator's explicit exception for `.githooks/pre-commit` and `.githooks/commit-msg`; do not replace or bypass
the gate to install this check. Until that decision, run the first two commands above before each commit.

The prepared installer is an unapplied, reviewable patch. After the coordinator authorizes those two paths,
install it with one command from the repository root:

```sh
git apply tools/privacy/hooks.patch
```

This adds the staged scanner before the existing fast gate and an executable commit-msg hook. It does not alter
the existing pre-push hook or Git configuration. Include the resulting hook-file changes in cross-vendor review
and the coordinator's integration. This task leaves the patch unapplied under the current reservation.

The intended pre-commit integration runs `--staged` before the existing `cargo xtask check --fast`. The commit-msg
hook passes its first argument to `--message` with `--git-message`: this reads `core.commentChar` via `git config`
(default `#`) and ignores lines starting with that marker and everything from its exact Git scissors line onward,
without rewriting the message file. For `auto`, the scissors line identifies Git's selected marker; without it,
ambiguous comment lines remain scanned to avoid hiding message content. Raw `--message` and
recorded CI commit messages are scanned without this cleanup. Both hooks propagate scanner failures.
G1/task 0043 can add the same
entry point to xtask later; this task does not edit xtask. CI runs the Python regression suite and scans the incoming
push or pull-request range. A new branch, missing before object or non-ancestor before object (force-push) uses
its merge base with the repository's default branch. CI fetches `deckerjulian/oos` branches into
`refs/remotes/upstream/*` without changing a configured remote, then excludes commits reachable from any of
those refs using `--exclude-remotes 'upstream/*'`. CI fails closed if upstream is unreachable: a failed fetch fails
the job, including before initial adoption. Fork additions and new merge-resolution lines
remain subject to scanning. The pull-request target SHA remains the policy/range base for pull requests.

All text input uses LF-delimited lines, stripping one trailing CR per line. Other control and Unicode separators
remain content, so they cannot remove an addition's patch prefix or shift merge line intersections. The same rule
applies to tree scans, messages and exact local literals; file reads preserve embedded CR characters.
