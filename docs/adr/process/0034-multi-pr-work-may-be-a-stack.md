# ADR 0034: Multi-PR work may be a stack

- Status: accepted
- Date: 2026-11-17
- Category: process

## Context

Rebase merges preserve topic commits. Splitting each step into a pull request adds review and CI work; dependent
topics sometimes benefit from separate reviews.

## Decision

- Default to one pull request with several topic commits when needed. Each commit stays a signed-off Conventional
  Commit of one topic, with fix-ups folded in before final review (ADR 0019 and ADR 0040).
- Multi-PR work may be a stack, maintained with `gh stack`: the bottom layer targets `main`, each higher layer the
  branch below it. Each layer is reviewable and green on its own; branches share the issue or task reference.
- Stacks land bottom up through the merge queue by rebase. Update them by rebase, never merge commits. A changed
  head needs renewed review. Only the stack's own branches may be pushed with `--force-with-lease`.
- Sequential pull requests, opened after their predecessors merge, are also allowed. Stacks require branches in the
  same repository; contributors without push access can submit sequential pull requests from their fork.

## Consequences

A single pull request keeps logical history without separate CI runs per topic. A stack permits separate reviews
but repeats checks and renews reviews after rebases. The operating commands and outage procedure live in
[CONTRIBUTING.md](../../../CONTRIBUTING.md#stacked-pull-requests).
