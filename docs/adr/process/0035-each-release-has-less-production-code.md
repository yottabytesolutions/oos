# ADR 0035: Each release strives for less production code than the one before

- Status: accepted
- Date: 2026-11-17
- Category: process

## Context

Per-file limits do not limit the size of the whole product. Reuse reduces how much code must be understood and kept
correct; a smaller line count alone does not prove that.

## Decision

- Each release strives for fewer production code lines than the previous release. Extend existing abstractions
  before adding parallel ones; reviewers flag duplication and missing reuse.
- A release that grows explains why in a `Code growth:` paragraph in its tag annotation or `CHANGELOG.md` section.
- Tests and documentation do not count toward the goal and must not be trimmed to meet it. File size limits also
  exclude comments, including doc comments; blank lines still count.
- Generated tables (ADR 0038) can shrink the count without shrinking complexity. Review the represented behaviour
  and the abstraction, rather than treating the count as proof of simplification.

## Consequences

Pull request counts are informational. The release check requires an explanation of growth. The counting
specification and commands live in [development.md](../../development.md#production-code-size).
