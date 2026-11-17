# ADR 0026: A contribution is signed off, holds no vendor material and enters main as reviewed commits

- Status: accepted, amended 2026-11-17 (how a contribution enters main)
- Date: 2026-11-16
- Category: process

## Context

ADR 0013 keeps vendor code and data out of the tree, and ADR 0015 asks every statement about the hardware to say how
it is known. Both were written for the files of the tree. Within one week pull requests from forks brought more than
250 commits (pull requests 7 to 13). Three things showed:

- Nobody had been asked where a contribution comes from. The licence note in `CONTRIBUTING.md` says under which
  licence the work is published, not that the contributor may publish it.
- The rules did not name the texts around the code: commit messages, pull request descriptions, issues. Working
  notes of a fork came in with its commits and had to be rewritten for the public tree afterwards.
- A merge keeps every commit of a pull request in the history of main, also the ones a later commit of the same pull
  request corrects. A review of the final state does not see them.

## Decision

- **Sign-off.** Every commit of a contribution carries a `Signed-off-by:` line with the name of the person who
  submits it (`git commit -s`). With it the person states the Developer Certificate of Origin 1.1
  (<https://developercertificate.org>): the work is their own or they have the right to submit it, under
  GPL-2.0-or-later. This holds for work an agent wrote as well; the person who submits it signs.
- **No vendor material, anywhere.** ADR 0013 holds for code, comments, documents, commit messages, pull request
  descriptions and issues alike. The hardware is described as behaviour, in the contributor's own words, with how it
  is known (ADR 0015). Material from the stock or a modified firmware stays out: code, listings, copied tables,
  strings, and the names of its functions, classes and files. What the vendor publishes for users is not meant: the
  command names of the programming guide, the manuals, the data sheets of the parts.
- **Other projects.** Code or text taken from another project names its source and its licence in the pull request,
  and the licence allows GPL-2.0-or-later.
- **One reviewed commit per topic.** A pull request from a fork enters main squashed: one commit per topic
  (ADR 0019), made from the state that was reviewed. The commits of the fork stay in the fork.
- A pull request or an issue that does not keep these rules is not merged, and its text is edited or removed.

## Alternatives

- **A contributor licence agreement.** More than this project needs: the licence stays GPL-2.0-or-later, and nobody
  hands over rights. The sign-off is what the Linux kernel uses for the same question.
- **Merging pull requests with their commits, as before.** Keeps the author's history, but puts unreviewed states on
  main for good. The author is still named: a squashed commit carries the author and the sign-off.

## Consequences

- A contributor adds `-s` to `git commit`; a pull request without sign-off waits until its commits have it.
- The history of main reads as one line of reviewed changes. The detailed history of a larger piece of work stays in
  the fork it came from; the pull request links it.
- The maintainer reads the commits and the texts of a pull request before taking it, not only the final diff.
- The pull request and issue templates state the rules, so nobody has to find them first.

## Amendment 2026-11-17: contributions to this fork

After the hard fork (ADR 0030) `main` of this tree is the trunk, and it welcomes outside contributions, by people and
by agents, under one structure:

- A contribution arrives as a pull request from a feature branch against `main`. One topic per pull request.
- The gate (`cargo xtask check`, ADR 0031) passes in CI before a merge. CI runs on GitHub-hosted runners and is set up
  in a later phase; until then the maintainer runs the full gate locally on the candidate.
- A pull request enters `main` squash-merged or rebase-merged: commits made from the reviewed state, with the author
  and the sign-off kept, in the commit style of ADR 0019 as amended.
- **Linear history.** `main` only moves by fast-forward. Rebasing is the default way to integrate. Merge commits are
  discouraged and allowed only where the second parent carries information, such as a long-lived import whose own
  history matters. Never merge `main` into a feature branch: rebase the branch onto `main`. Everyone pulls with
  rebase (`git pull --rebase`; `git config pull.rebase true` and `git config rebase.autoStash true` set it once).
- **Force-push.** A feature branch may be force-pushed with `--force-with-lease`; that is what feature branches are
  for. `main` is never force-pushed.
- **Branch names.** `<type>/<ref>-<slug>`. `type` is one of `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `ci`,
  `build`, `chore`. `ref` is a GitHub issue number or a task id `t<NNNN>` from `docs/tasks/`. `slug` is lower case,
  words joined by `-`. Example: `fix/t0091-trigger-holdoff`, `feat/42-uart-decoder`.
- `main` is the only long-lived branch. The old `overhaul` branch is retired; it becomes the annotated tag
  `archive/overhaul`.
- The sign-off (DCO) stays: every commit of a pull request carries `Signed-off-by:`. The maintainers' own integration
  commits made before the pull request flow is in place are not signed off.
- The vendor-material and other-project rules above are unchanged.
- CI enforcement of branch names and linear history, release tags, pull request templates and required checks follow
  in a later phase and amend this record.

The text above that speaks of "pull requests from forks" entering upstream's main is replaced by this section.

## References

[CONTRIBUTING.md](../../../CONTRIBUTING.md), [AGENTS.md](../../../AGENTS.md),
`.github/PULL_REQUEST_TEMPLATE.md`, `.github/ISSUE_TEMPLATE/`, ADR 0013, ADR 0015, ADR 0019.
