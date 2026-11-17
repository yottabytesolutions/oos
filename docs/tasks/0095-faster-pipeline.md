---
id: 95
title: Faster CI pipeline with fewer external actions
kind: chore
status: active
release: 0.2.0
owner: agent
depends: []
---

## Goal

Shorter wall time for the required checks of a pull request and for the nightly gate, on free GitHub-hosted runners,
without a required check that can pass unseen. Fewer third-party actions. A working image job.

## Plan

Reserved paths: `.github/workflows/pr.yml`, `.github/workflows/main.yml`, `.github/workflows/release.yml`,
`.github/workflows/image.yml`, `.github/workflows/privacy.yml`, `tools/ci/changes.py`, `tools/ci/run_summary.py`,
`tools/ci/kernel-base.sh`, `tools/ci/pr_meta.py`, `tools/ci/apply-repo-settings.sh`, `tools/tests/test_changes.py`,
`tools/tests/test_run_summary.py`, `tools/tests/test_pr_meta.py`, `docs/development.md`, `kernel/README.md`.

- Path filter (`tools/ci/changes.py`, tests): documents, GitHub settings and CI scripts skip the Rust jobs; every
  required job still reports. Merge-queue groups with the PR head's tree skip checks that passed there.
- Release tests per PR without LTO; LTO release tests, coverage and doctor nightly. Debug info line tables only.
- Caches saved only by the push to main, which builds tests without running them.
- Nightly gate as parallel `cargo xtask step` jobs.
- cargo-deny and git-cliff through taiki-e/install-action; allow list without their actions.
- pr-meta: only `ubuntu-24.04`, `ubuntu-24.04-arm`, `ubuntu-latest`.
- Kernel base as the pre-release `kernel-base-6.12.115` (`tools/ci/kernel-base.sh`).
- Job and step durations in the run summary; before and after timings in the PR.

Not done, with reasons: cargo-nextest (the test step and its counts are defined in xtask, ADR 0031; a runner change
is a gate change); mold (the pinned toolchain already links with rust-lld on x86-64 Linux, and `RUSTFLAGS` would
replace the scope target's flags in `.cargo/config.toml`); splitting clippy into host and aarch64 jobs (the clippy
ratchet counts a finding seen on both targets once; split runs would need an xtask change of the ratchet).

## Status

- 2026-11-17: branch `ci/t0095-faster-pipeline` from main 9072ea2. Pre-release `kernel-base-6.12.115` published,
  asset sha256 matches `kernel/base.sha256`.
