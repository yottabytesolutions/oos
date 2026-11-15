# Delivery

How work flows from a task to a published branch while the overhaul is under way. [Review](review.md) and
[integration](integration.md) define the rules; this page sets the pace. It overrides them where it says so.

## One job per task

A task is one worker job: plan and implement together. The worker writes the plan into the task file first,
then implements it in the same job. A separate plan job and plan review happen only for migration steps that
change the architecture (new crates, ownership, change detection); those plans get an architecture review.

The brief names the minimal scope, the files other tasks hold and the review tier. Work outside the scope is a
follow-up task, not part of the change. A worker may touch further files the deliverable strictly needs when no
other task holds them, and lists them in the task Status.

## Review by risk

| Tier | Changes | Rounds |
|---|---|---|
| Unit | Code that can damage or brick a scope: start-up and exit, updater, boot, acquisition, FPGA, kernel | Until approved |
| Standard | Application code, drawing, remote interfaces | One, then one verification round for P1 and P2 fixes |
| Tooling | Gate, scripts, docs, task files | One; P3 findings become follow-ups |

The cross-vendor rule stays: the reviewer is never from the author's vendor. A P1 always blocks.

## Gates

Workers run the fast gate (`cargo xtask check --fast`) and the tests of what they changed. The coordinator runs
the full gate once per integration batch, not once per task. Full gates run one at a time on a host: parallel
runs distort the performance guards. Until the 0.2.0 release the local full gate is the only gate; repository
CI is switched off.

## Integration batches

Reviewed branches collect on an integration branch from `overhaul`. One full gate covers the batch; on success
`overhaul` fast-forwards to it. A failing batch is bisected and the failing branch goes back to its author.
Task files move to `done/` in the batch. Worktrees of merged branches are removed with the batch.

## Publication and unit rounds

The coordinator publishes `overhaul` once per batch through the scrub. Unit rounds follow the migration plan's
milestones; a change of the unit tier that the plan gives no round gets a short smoke round under a scope lease.
