# Integration

## Roles

- **Owner:** the human responsible for direction, acceptance and permission for hardware installs and publication.
- **Coordinator:** the agent session responsible for planning, assignments, integration and publication.
- **Author:** whoever wrote the change. Lands it through the merge queue once the required checks and Codex pass.
- **Reviewer:** an agent of another vendor. For human-authored changes, any reviewer who did not write it.

Use the [task claim](../tasks/README.md#claims) before editing. CODEOWNERS assigns reviewers, not edit rights.

## Merge order

The [migration specification](../architecture/migration.md#steps) defines the order and acceptance tests.
Land guardrails first at gate step G1, as specified in the migration steps. M0 is already done.
The tables below summarize step groups. Use the migration steps for numbered parts and prerequisites,
including M1b, M5b and G1 before M2.1. One branch implements one step or numbered part.
Parallel preparation does not waive a predecessor's acceptance or permit an early merge.

| Steps | Dependency and integration boundary |
|---|---|
| M1 | Capture baseline golden pictures before M2 changes behaviour. |
| M2, M3 | Follow [serial migration work](#serial-migration-work) and the migration prerequisites. |
| M4 | Needs M2 generations and M3's acquisition split. Complete unit round 1. |
| M5, M6, M7 | Replace shared channels, separate measurement reads, then replace lines. Complete unit round 2. |
| M8, M9 | Add dots, then whole-batch accumulation. Complete unit round 3. |
| M10 | Builds on M9's accumulator and window. Complete unit round 4. |
| M11, M12 | Add sparse pacing, then remove old drawing paths after acceptance. Complete unit round 5. M12 is the stable stop point. |
| M13, M14, M15 | Group settings, add commands, then move remote adapters to commands and queries. |
| M16, M17 | Replace the step with stages, then extract components and delete `Ui`. |
| M18 | Move to the target layout last. Complete unit round 6 against M12's results. |

## Parallel work

Use the [module map and layers](../architecture/README.md#module-map) to split responsibilities.
Check both file overlap and shared contracts. Reserve tests and wiring as well as implementation files.
The coordinator confirms disjoint accepted claims before parallel work starts.

| Candidate work | Conditions |
|---|---|
| M2 change detection and M3 acquisition split | Follow [serial migration work](#serial-migration-work). Use disjoint claims and an agreed Board contract. M2 owns change wiring and model moves; M3 owns hardware-loop edits. Shared wiring waits. |
| M7 or M8 kernel preparation and M11 pacing preparation | Picture kernels and the acquisition loop may be disjoint. Shared model fields, UI wiring and tests wait for their turn. M11 still merges after M10. |
| M13 settings parts or M17 view parts | Separate groups or components may proceed together once their contracts exist. Central dispatch, legacy type removal and shared wiring are serial. |
| M15 transport preparation alongside M13 or M14 | Only parser and in-memory transport work against an agreed command/query contract. Command application and main-thread integration wait for M14. |

These are conditional opportunities, not permission to overlap claims. Otherwise work serially.
Steps touching `ui/src/ui.rs`, `ui/src/board/dho/hw.rs` or `ui/src/model.rs` reserve each file explicitly.
Only one active writer may hold a given hot file. M2 to M6 share change, acquisition and UI wiring.
M7 to M12 can share drawing dispatch and UI wiring. M13 to M17 share model and UI removal work.
Serialize those edits even when the underlying modules differ.

### Serial migration work

The oos-core crate creation part of M2 runs alone, like M18. Finish or hand off other migration work first.
M3 parallel work waits until the crate creation part merges. M18 moves invalidate paths and claims across the tree.

## Review and merge

Follow the [review protocol](review.md) before merging.
Record evidence in the task file's Status lines and the merge commit body.
The [PR template](../../.github/pull_request_template.md) is the field list. Use it in the PR when one exists.
History is linear ([ADR 0026](../adr/process/0026-contributions-are-signed-off-and-hold-no-vendor-material.md)):
branches are rebased onto `main`, never updated by merging `main` into them, and `main` only fast-forwards.
Never rewrite commits on `main`. Changes after review, including rebases with conflict resolutions, need renewed
independent review.

The coordinator pauses a conflicting merge. The affected authors resolve textual and semantic conflicts.
Do not guess another area's intended behaviour.
Review the resolution with the affected area reviewers before integration.

The merge queue is the integration step. It re-runs the required checks on top of the current `main` and
lands the pull request by rebase. The required checks do not run every step (the coverage floor runs only nightly),
so the author runs the full `cargo xtask check` before marking a pull request ready. The author enables auto-merge
once the required checks are green and Codex has no unresolved findings
([CONTRIBUTING](../../CONTRIBUTING.md#review-gate)). A stack lands bottom up
([ADR 0034](../adr/process/0034-multi-pr-work-lands-as-a-stack.md)). Paths in CODEOWNERS also need the owner's
approval, which the ruleset enforces. Changes that need unit evidence carry `needs-unit` until the owner has seen
it. After the merge, the owner accepts or reverts the change.

## Hotfix lane

Fix an urgent unit regression on the unit branch first. Keep the fix small and independently reviewed.
Add a regression test that fails before the fix and passes after it. Run that branch's checks.
Hardware access requires the owner's task-specific go. Record lease fields once task 0021 lands;
until then the owner's task-specific go applies. Record the installed build and observed result.
Do not infer installation or recovery permission.

Forward-port the regression test plus the fix to the integration branch. Adapt it to the current owners and ports.
Show the test failing there before the fix and passing after it. Link both revisions and their evidence.
Run independent review and the full integration gate again. A unit-branch fix alone does not close the hotfix.
