# Contributions preserve authorship, privacy and linear history

- Number: 0003
- Category: policy
- State: in force
- Date: 2026-11-18

## Context

Contributions arrive from people and their coding tools, so review needs a traceable statement of submission rights.
Squash merges made the merging account the author and added a co-author trailer. Rebase preserves the reviewed topic
commits and their sign-offs without publishing private device data.

## Decision

Every commit carries the submitter's `Signed-off-by:` under DCO 1.1, including work written by an agent. There is no
contributor licence agreement. Material from another project names its source and GPL-compatible licence. No serial
numbers, MAC addresses, LAN addresses, licence keys or calibration data enter the tree.

Pull requests land by rebase through the merge queue, with reviewed topic commits unchanged. No squash, merge commits or
force push to `main`. CODEOWNERS paths need the maintainer's approval.

Check: pr-meta CI validates sign-off and Conventional Commits; privacy CI scans for private data; the main ruleset
enforces linear history, rebase and code-owner review.

## Alternatives

A CLA: rejected because GPL licensing needs no transfer of rights; DCO records the right to submit. Squash: rejected
after it replaced authorship and added co-author trailers. Merge commits: rejected because they retain intermediate
states that a review of the final diff does not inspect.

## Consequences

Contribution procedures live in CONTRIBUTING.md. A green gate does not replace required review.
