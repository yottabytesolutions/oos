---
id: 66
title: Merge upstream (PR 7, PR 8, main) into overhaul
kind: chore
status: active
release: none
owner: claude
depends: []
---

# Merge upstream (PR 7, PR 8, main) into overhaul

## Goal

Bring upstream's work since our scrubbed PR 7 into `overhaul`: the PR 7 merge, PR 8 and upstream `main`. Keep our
acquisition correctness work and both draw paths. Review tier: Unit (acquisition and start-up code change).

## Plan

Reserved paths: whole tree on branch `integ-20-upstream` (integration merge), this task.

1. Bridge: `git merge -s ours` of upstream's PR 7 head, the scrubbed copy of our private branch at a commit that is
   already an ancestor of `overhaul`. The tree does not change; it gives upstream and `overhaul` a merge base.
2. Three step merges: the PR 7 merge, the PR 8 merge, upstream `main`. Each upstream point is first formatted with
   our `rustfmt.toml` on a temporary local branch, so conflicts show content changes only. One merge commit per step
   with its notable resolutions.
3. Port upstream's `model.rs` changes into `core/` (M2.1 moved the model there), separate commit.
4. Ratchet rebaseline for upstream-added code only, one commit, with a summary of what grew.
5. Golden pictures: run, re-bless only where upstream's intended drawing changes alter them, separate commit listing
   each case and the upstream commit.
6. `cargo xtask check --fast` after each step, `cargo xtask check` at the end.

Owner decisions applied:

- Both draw paths stay: upstream's Standard (multi-core fast path, default) and our batch path, with upstream's
  "Acquire > Draw path" setting. adr/0013 and migration M9/M12: removal superseded, M12 measures both on the unit.
- Our acquisition correctness work stays (M2.x change detection and generation filter, kexec and watchdog, PCIe
  Gen1, self-heal, update hardening, SCPI empty reply and compound fixes). Where upstream fixed the same, one
  implementation stays; the Status names which.
- Upstream's module registries (decoders, measurements, analysers; their ADR code/0002) stay as they are.
- Modify/delete: upstream deleted fork specs, the unit doc and the unit rootfs files; ours stay (private, scrubbed
  at publish). Overhaul deleted per-crate `Cargo.lock` files and `docs/architecture.md`; overhaul's side stays.
- Open owner questions are recorded below and resolved conservatively (keep both).

## Status

- 2026-11-16: claimed on `integ-20-upstream` from `overhaul` c6f1f98.
