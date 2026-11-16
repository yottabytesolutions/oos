---
id: 84
title: Merge upstream 15b819e2 (v0.1.13 and review fixes) into overhaul
kind: chore
status: active
release: none
owner: claude
depends: [66]
---

# Merge upstream 15b819e2 (v0.1.13 and review fixes) into overhaul

## Goal

Bring upstream `main` 15b819e2 (72 commits since 2bd51214) into `overhaul` and port the behaviour that lands in
code our architecture moved. Review tier: Unit for the batch frame 0 and frame 1 port, host for the rest.

## Plan

Reserved paths: whole tree on branch `integ-27-upstream` (integration merge), this task.

1. Merge through copies of the merge base and upstream formatted with our `rustfmt.toml` (method of task 0066).
   Second parent is the unformatted upstream commit. Process files and lint tables resolve to ours, with the
   upstream rules that bind upstream-bound work added and cited by path. `docs/architecture.md` stays deleted.
2. Port commits, each with a test that fails without it:
   1. Trigger level order as a core normalisation rule (b90facd5), no `mark_changed`/`settings_changed` calls.
   2. Batch frame 0 and frame 1 without edge (0cc8fcb6) in `batch::Readout` and `record_decode`; 71034ebc dropped.
   3. `Shared` lock not held through a raw decode (3930159f).
   4. STOP statistics rules (371f82bc, 42e6e5d6), STOP raw and pairs split (b19a56ad) through the accumulator;
      tests of d9b2779b, e7302137 and the LA case of 1a457e30.
   5. `net.allow` live (c24899c4, 256a48d4) as a command plus the system adapter.
   6. Reference and SIM session restore on our seam.
   7. Un-ignore the t0078 dsp tests upstream fixed (Hall, MLT-3, rail).
   8. `Item::RemAllow` gets the committed-choice highlight of task 81.
3. Separate commit: remove per-unit ADC core trims and local paths from `docs/fork/core-offset.md`.
4. Ratchet rebaseline for upstream-added code only, through the pending-rebaseline mechanism.
5. Full gate, `cargo xtask check --image`, then `cargo xtask ratchet` and "Tighten ratchets after integ-27".

## Status

- 2026-11-16: paused, disk full (925 MiB free). Merge of 15b819e2 resolved in the working tree, not committed
  (MERGE_HEAD set; formatted method: base 7f7adc95, upstream 82cff324 fetched from scratchpad/fmtclone). 29 paths
  conflicted; all resolved; `cargo check --workspace --all-targets` clean, `cargo fmt` applied. Fast gate before the
  merge commit: size, statics, unsafe (upstream findings, pending rebaseline once committed). Next: debug test run
  (killed by ENOSPC), commit the merge, then ports 1 to 8. Prepared: core-offset doc patch
  (scratchpad/t84/coreoffset.patch), upstream frame-0 test (scratchpad/t84/frames_before_first.rs).
- 2026-11-16: claimed on `integ-27-upstream` from `overhaul` 8fafc719.
