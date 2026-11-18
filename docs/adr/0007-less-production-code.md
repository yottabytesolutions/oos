# Each release has less production code

- Number: 0007
- Category: policy
- State: in force
- Date: 2026-11-18

## Context

Per-file limits do not limit the size of the whole product: splitting a growing implementation across files still passes
them. Repeated dispatch and parallel implementations increase the code that must be understood and kept correct. A
release-level count makes reuse visible, while review must still judge behaviour and complexity.

## Decision

Every release has fewer production code lines than the previous release, unless its release notes contain a `Code
growth:` paragraph approved by the owner. docs/development.md defines the count; `tools/ci/code_growth.py` computes the
release comparison. Tests and documentation never count and are never trimmed to satisfy this rule. Generated tables
count as written (ADR 0029).

Check: release workflow and code_growth.py enforce the paragraph; review checks owner approval and missing reuse.

## Alternatives

Per-file limits alone: rejected because total code can grow through more files or parallel implementations. An absolute
ban on growth: rejected because necessary features may justify an owner-reviewed release exception.

## Consequences

A release that grows waits for the owner-approved exception. Tooling size is reported separately.
