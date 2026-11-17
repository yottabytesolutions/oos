# ADR 0034: Work that spans more than one pull request lands as a stack

- Status: proposed
- Date: 2026-11-17
- Category: process

## Context

ADR 0026 asks for one topic per pull request, and the pull request rules keep each one at about 400 changed lines.
A refactor or a feature that needs several steps then becomes several pull requests that depend on each other. Until
now the second step waited for the first to merge, or was opened against `main` with the first step's commits in its
diff. The first stalls the work. The second makes the reviewer read the same change twice and hides which commits
belong to which topic.

GitHub supports stacked pull requests: a chain of pull requests in one repository where the bottom one targets the
trunk and each higher one targets the branch below it. The `gh stack` extension of the GitHub CLI creates, rebases,
pushes and links them. The merge requirements of every pull request in a stack are those of the bottom one's base
branch, the checks of every layer run, and a stack merges from the bottom up, through the merge queue where the base
has one. Cross-fork stacks are not supported: every branch of a stack lives in the same repository.

## Decision

- Any contribution that spans more than one pull request is a stack, built and kept in order with `gh stack`.
- Each layer is one topic, reviewable on its own and green on its own: every required check passes on every layer.
- Each layer's branch keeps the scheme `<type>/<ref>-<slug>` (ADR 0026). The layers of one piece of work share the
  `ref`, so `refactor/42-split-bus` and `test/42-bus-fakes` belong together.
- A stack merges bottom up. Where the base is `main` that is through the merge queue, as a rebase merge: each
  layer's commits land on `main` as they are, so each commit is a clean, signed-off Conventional Commit of one topic,
  with review fix-ups folded into it (`git rebase -i --autosquash`) before the last review. `gh stack merge` queues the
  layers in order. The layers above are rebased onto the new base, never merged.
- Updates use `gh stack sync` or `gh stack rebase`, not merge commits. Force-pushing the stack's own branches with
  `--force-with-lease`, which `gh stack push` and `sync` do, is allowed; `main` is never force-pushed.
- Agents use stacks for refactors and for any change their issue splits into steps.
- Limits: all layers live in this repository. An outside contributor without push access submits one layer at a
  time from a fork, the next after the previous one merged; or the maintainer grants push access, or recreates the
  stack from the fork's branches.

## Alternatives

- **One large pull request.** One review of everything at once; findings in one topic block the others, and the
  history mixes several topics in one review against ADR 0019.
- **Dependent pull requests against `main` without a stack.** Each later pull request repeats the earlier diffs until
  they merge, and nothing keeps the order or retargets the bases.
- **Waiting for each step to merge before opening the next.** Correct, but it serializes review and work.

## Consequences

- A reviewer reads each layer as its own diff, and a finding is fixed in the layer it belongs to; the layers above
  are rebased with `gh stack rebase --upstack`.
- Every rebase gives the layers above a new head commit, so their `codex-review` status is pending again until
  Codex reviews the new head (`@codex review`).
- The ruleset bypass for a Codex outage merges only the bottom layer of a stack, never the whole stack at once; an
  outage merge goes layer by layer.
- Contributors need `gh` 2.90 or later with the `github/gh-stack` extension; plain Git still works for a single
  pull request.
- Worth a second look when GitHub supports cross-fork stacks, or when the merge queue and stacks interact in a way
  that loses a check.

## References

[CONTRIBUTING.md](../../../CONTRIBUTING.md#stacked-pull-requests), [AGENTS.md](../../../AGENTS.md),
<https://docs.github.com/en/pull-requests/get-started/about-stacked-prs>,
<https://docs.github.com/en/pull-requests/reference/stacked-prs-cli-commands>,
<https://docs.github.com/en/pull-requests/reference/stacked-pull-requests>, issue #11, ADR 0019, ADR 0026,
ADR 0031.
