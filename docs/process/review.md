# Review

Use the [roles](integration.md#roles) for owner, coordinator, author and reviewer.
Every change needs an independent adversarial review before merge.
Agent-authored changes need review by an agent from another vendor.
Pair Codex authors with Claude reviewers and Claude authors with Codex reviewers.
Other vendors pair across vendors too. For human authors, use any reviewer who did not write the change.
If a suitable reviewer is unavailable, hold the merge. Self-review and green checks do not satisfy this rule.

Follow the [merge and acceptance order](integration.md#review-and-merge).
CODEOWNERS is a commented area map until the owner enables real teams.
Area reviewers supply specialist review. They do not gain edit rights or merge authority.

## Protocol

The author supplies the fields in the [PR template](../../.github/pull_request_template.md), task claim,
exact candidate revision and relevant migration step. Evidence lives in the task file's Status lines
and the merge commit body. Use the template in the PR when one exists.
Include the parent revision, changed boundaries, all applicable inventory IDs/results and complete logs.
Workers supply fast-gate and focused-test results. The coordinator supplies the default full-gate
result for the integrated batch, and `--image` evidence before unit rounds and releases.
Repository CI is off until 0.2.0. Do not combine evidence from separate revisions. Missing
prerequisites and not-selected checks remain explicit. The coordinator arranges independent review.
For refactors, name the existing tests and golden pictures that must stay unchanged.

The reviewer reads the task, applicable [migration step](../architecture/migration.md#steps),
[layers](../architecture/README.md#layers) and area specification. Inspect the diff and surrounding callers.
Challenge the ownership model and try to disprove the claimed behaviour with a counterexample.
Flag duplicated logic, near-copies and missing reuse as findings. Ask for an existing abstraction to be extended
instead of a parallel one, and for deleted tests or comments that only shrink the line count to be restored
([ADR 0007](../adr/0007-less-production-code.md)).
Check the evidence and reproduce relevant host tests. Record anything that could not be verified.

Return findings first, ordered by severity. End with approve or request changes for the exact reviewed revision.
Record reviewer identity and vendor, author identity and vendor (or human), commands run and remaining limits.
The author addresses each finding with evidence. The reviewer verifies the fixes and changed assumptions.
New commits or conflict resolutions need renewed review. Follow the [integration protocol](integration.md#review-and-merge).

## Architecture checklist

- **Boundaries:** follow the layers and module map. The domain does no IO, clock reads or threading.
  Adapters do not call each other. Only `board/dho` uses `oos_hal::dho`; only display owns DRM, GPU and VPU access.
- **Settings:** writes go through commands applied by the change stage. Internal writers use the next-step outbox.
  Normalize before classify. New settings have meaningful classes and mapping fields enter `MappingKey`.
- **Acquisition:** frames convert from their own `BatchMeta`, never current settings. Check `gen`, `epoch`,
  `stats_gen` and mapping-key filtering. An epoch-only boundary drops old work. Unresolved tokens accept nothing.
  Remote waits use the required generation and epoch.
- **Picture:** only the accumulator writes maps, weights and window. Consumers read `&Picture`.
  Measurement reads never draw. Whole batches stay whole through accumulation.
- **Main thread:** no blocking IO, socket waits or large reply encoding. Check slow clients, queue pressure,
  lock scope and worker handoffs against [threads and messages](../architecture/README.md#threads-and-messages).
- **New state:** state its owner, lifetime and maximum bytes. Bound queues, caches and snapshots in bytes,
  including retained and in-flight payloads. Explain overflow, eviction or backpressure and test the bound.
  Check the [performance budgets](../architecture/README.md#performance-budgets).
- **Unsafe:** allow it only in `oos-hal`, the adapters `board/dho`, `display`, `input`, `remote`, `system`,
  `instruments`, and SIMD kernels in `picture/deposit`.
  Match the allowed paths in `xtask/data/architecture.txt`. Every unsafe block or function explains its invariant
  with `// SAFETY:`. Never add unsafe to `oos-core` or another stage.
- **Regression tests:** a bug fix has a test that fails on the parent and passes after the fix.
  Verify commands and results. For a pure refactor, verify named existing tests and unchanged golden pictures.
  Check debug and release results and required performance evidence.
- **Deletions:** search for the legacy names listed by the migration step. Confirm old paths are gone.
- **Golden pictures:** drawing changes include golden pictures. Each changed reference has a reason in the
  commit message. Check the actual image difference and tolerance, not only a passing comparison.
- **Hardware:** if the unit was used, require the owner's task-specific go, installed build, setup,
  observations and artifacts. Record lease fields once task 0021 lands; until then the owner's task-specific go applies.
  Distinguish verified results from assumptions. Check safe failure and recovery
  behaviour against AGENTS.md. No unit use means an explicit statement that hardware was not verified.
- **Guardrails:** do not weaken lints, limits, coverage or architecture rules. Check any claimed owner-approved ADR.
  The full gate must pass again on the integrated result.

## Findings

Use `severity | file:line | finding | consequence | required evidence or fix`.
Give a repository-relative location for every finding. Cite the violated rule and a concrete failure case.

| Severity | Meaning | Merge action |
|---|---|---|
| Critical | Unit safety, data loss or a security breach. | Block. Resolve and review again. |
| High | Wrong behaviour, broken ownership or an architecture boundary violation. | Block. Resolve and review again. |
| Medium | Missing evidence, an uncovered edge case or a maintainability defect. | Resolve before merge, or record explicit human acceptance and a claimed follow-up task. |
| Low | A minor clarity or style issue. | Record its disposition. |

Acceptance requires independent review, a passing full gate and all required hardware evidence, regardless of severity.
A review with no findings still states its scope and verification limits.
Record the acceptance and merge decisions under the [integration protocol](integration.md#review-and-merge).

## Area map

[CODEOWNERS](../../.github/CODEOWNERS) is the source for area assignments in the current and target layouts.
Unlisted paths use its architecture catch-all. Later specific rules take precedence.
The owner enables the commented rules when real teams exist. No live handles belong in the published file.
