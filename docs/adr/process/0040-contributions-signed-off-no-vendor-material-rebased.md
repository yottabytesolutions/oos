# ADR 0040: A contribution is signed off, holds no vendor material and lands rebased through the merge queue

- Status: accepted
- Date: 2026-11-17
- Category: process

## Context

The rules for a contribution were spread over records. ADR 0026 added the sign-off, extended the vendor rule to every text around the code, and
described how a contribution from a fork entered upstream's `main` as squashed commits. Its 2026-11-17 amendment
replaced that flow after the hard fork (ADR 0030) and left steps marked "a later phase" that are now done: CI runs
the gate, the merge queue lands pull requests, the rulesets enforce the checks.

This record carries them as they hold now. What may not enter the tree from the vendor's firmware is ADR 0013.

## Decision

- **No vendor material.** ADR 0013.
- **Nothing private.** No serial numbers, MAC addresses, LAN addresses, licence keys or calibration data.
  `tools/privacy/scan.py` checks it in the pre-commit hook and in CI.
- **Other projects.** Code or text taken from another project names its source and licence in the pull request, and
  the licence allows GPL-2.0-or-later.
- **Sign-off.** Every commit carries `Signed-off-by:` of the person who submits it (`git commit -s`), stating the
  Developer Certificate of Origin 1.1. This holds for work an agent wrote: the person who submits it signs.
- **Pull requests.** `yottabytesolutions/oos` is the single repository. GitHub issues are the work queue; an issue
  ready for an agent carries the label `agent-ready` and its acceptance criteria, and `docs/tasks/` holds only
  multi-step designs. One topic per pull request, from a branch `<type>/<ref>-<slug>` against `main` (`ref`: an issue number or `t<NNNN>`). Work that
  spans several pull requests is a stack (ADR 0034).
- **Merging.** A pull request lands when the required checks pass and the Codex review has no unresolved finding.
  Paths in `.github/CODEOWNERS` (boot, update, kernel, FPGA, acquisition, the workflows) also need the maintainer's
  approval. The author enables auto-merge; the merge queue rebases the commits onto `main` as they are, with their
  authors and sign-offs. Squash merges are off.
- **Linear history.** `main` only fast-forwards and is never force-pushed. Branches are rebased, never updated by
  merging `main`. A feature branch may be force-pushed with `--force-with-lease`. Fix-ups are folded into their topic
  commit before the pull request is ready.

## Alternatives

- **A contributor licence agreement:** more than the project needs. The licence stays GPL-2.0-or-later and nobody
  hands over rights. The sign-off is what the Linux kernel uses for the same question.
- **Squash merges:** they made the merging account the author and added a `Co-authored-by:` trailer.
- **Merge commits:** keep every intermediate state on `main`; a review of the final diff does not see them.

## Consequences

- Every commit on `main` is a reviewed, signed-off topic commit in the style of ADR 0019.
- A pull request or issue that breaks these rules is not merged, and its text is edited or removed.

## References

[CONTRIBUTING.md](../../../CONTRIBUTING.md), [AGENTS.md](../../../AGENTS.md), `.github/CODEOWNERS`,
`.github/rulesets/`, `tools/privacy/`, ADR 0013, ADR 0026 (superseded), ADR 0015, ADR 0019, ADR 0030, ADR 0034,
ADR 0037.
