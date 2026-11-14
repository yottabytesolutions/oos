---
id: 28
title: "Make package signing explicit"
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

# Make package signing explicit

## Goal

Keep ordinary builds from silently selecting a private signing identity.

## Plan

Reserved paths: `boot/build-dev-image.sh`, `boot/sign-dev-image.sh`, `boot/build-gel.py`,
`fpgad/src/bin/oos-update.rs`, `fpgad/tests/explicit_signing.rs`, `docs/update.md`,
`docs/tasks/0028-explicit-signing.md`, `README.md`, `docs/fork/unit-dho804.md`,
`keys/update.pub`, `.github/workflows/image.yml`, `tools/gel-sim.sh`, `COMMIT_MSG.txt`.

Before parallel migration, separate build, package and sign. Remove automatic private-key discovery.
Require explicit key selection and document public development fixtures and the bench trust policy.
A signature does not authorize installation on the shared unit. Audit item: 19.

Keep image assembly in the build step. Save its build id for a separate signing command.
Package the signed update as GEL only on request. Require explicit selection in the host CLI too.
Generate temporary test keys and verify signed packages with HAL's device verifier, offline.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

Repair review findings in CI packaging and signing input validation. Reject trusted public keys used as seeds.
Require explicit opt-in for untrusted signing identities. Restrict wrapper arguments and pack-only flags.
Add host regressions, run the requested offline checks, and leave a proposed commit message without Git writes.

## Status

- 2026-11-14: review fixes applied and left uncommitted for the owner. `COMMIT_MSG.txt` has a proposed
  imperative subject (66 characters) and rationale. The task stays active pending integration checks.
  In `fpgad`, `cargo test --offline` and `cargo test --offline --release` each exit 0.
  Both print `9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`,
  `3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`, and
  `14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
  In `hal`, `cargo test --offline update::` exits 0 and prints
  `67 passed; 0 failed; 2 ignored; 0 measured; 260 filtered out`.
  Its seven integration suites run 0 tests; their filtered counts are 4, 1, 2, 2, 2, 1 and 2.
  `bash -n boot/sign-dev-image.sh tools/gel-sim.sh` and `git diff --check` exit 0.
  Added-text style/privacy and commit subject checks exit 0.
  `CARGO_NET_OFFLINE=true cargo xtask check` exits 101: `error: no such command: xtask`.
  The gate and development guide are absent. Full gate and independent review remain integration checks.
  No network, device access, image build, commits, pushes or Git metadata writes.
- 2026-11-14: restored CI signing and GEL packaging, with checks for each required artifact before upload.
  Rejected trusted public keys as seeds and required `--untrusted-ok` for other untrusted identities.
  Restricted wrapper arguments and signing flags. Corrected the build id and simulator guidance.
  The regression run before fixes (`cargo test --offline --test explicit_signing` in `fpgad`) exited 101:
  `6 passed; 7 failed; 0 ignored; 0 measured; 0 filtered out`. The missing-build-id regression already passed.

- 2026-11-14: review follow-up claimed by codex on `t0028-explicit-signing`, base `9f03c97`.
  The owner reassigned this worktree after the previous author finished. Reserved paths include the workflow
  and GEL simulator. No network, device access, commits, pushes or Git metadata writes are authorized.

- 2026-11-14: implementation done as requested. Owner: codex. Changes remain uncommitted in this worktree.
  `COMMIT_MSG.txt` contains the proposed message. No network, device contact or Git metadata writes.
  `cargo test --offline --manifest-path fpgad/Cargo.toml` and its `--release` run exit 0.
  Both print 9 passed, 3 passed and 7 passed, with no failures or ignored tests.
  `cargo test --offline --manifest-path hal/Cargo.toml update::` and its `--release` run exit 0.
  Both print `67 passed; 0 failed; 2 ignored; 0 measured; 260 filtered out` for the update suite.
  Shell and Python syntax, signer help, added-text privacy/style and diff whitespace checks pass (exit 0).
  Full gate: `CARGO_NET_OFFLINE=true cargo xtask check` exits 101: `error: no such command: xtask`.
  The gate, hooks and development guide are absent on this branch. Full gate and independent review remain
  integration checks. No image build or hardware validation was attempted. Keep this file here until merge.
- 2026-11-14: separated image assembly, update signing and GEL packaging. Saved the image's build id.
  Removed all signing-key discovery. Updated the key policy and usage examples.
  Seven host checks pass with temporary keys and the device verifier. The no-selection regression fails
  against the original signer (exit 101) and passes after the fix.
- 2026-11-14: claimed by codex on `t0028-explicit-signing`, base `589ccb8`.
  The owner assigned this task and worktree directly. No Git metadata writes, network or device contact.
- 2026-11-14: seeded from task 0001. Not yet claimed.
