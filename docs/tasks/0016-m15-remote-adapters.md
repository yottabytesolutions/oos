---
id: 16
title: "M15: remote adapters on commands and queries"
kind: feature
status: active
release: 0.3.0
owner: cx-m15a
depends: [15]
---

# M15: remote adapters on commands and queries

## Goal

Complete [migration M15](../architecture/migration.md#m15-remote-adapters-on-commands-and-queries). The migration document is the specification.

## Plan

Reserved paths: `ui/Cargo.toml`, `Cargo.lock`, `ui/src/netio/`, `ui/src/netport.rs`, `ui/src/netport/`,
`ui/src/scpi.rs`, `ui/src/scpi/`, `ui/src/ui/graphics.rs`, `tools/tests/fixtures/perfmap/state.txt`, `ui/src/ui/scpi_operations.rs`, `ui/src/system/intake.rs`,
`ui/src/system/wake.rs`, `ui/src/system/device/remote.rs`, `ui/src/system/serve.rs`, `ui/src/main.rs`,
`ui/src/http/listener.rs`, `ui/src/net.rs`, `ui/src/vxi11.rs`, `ui/src/hislip.rs`, `ui/src/perfguard.rs`, `ui/src/perfguard/ring.rs`, `xtask/data/architecture.txt`, `xtask/src/arch/`, `docs/architecture/README.md`,
`docs/adr/code/0037-crates-where-they-remove-code-remote-io-on-tokio.md`, `docs/architecture/migration.md`,
`docs/tasks/0016-m15-remote-adapters.md`.

M15a (#138): extend the owned intake seam, implement raw framing/output on one current-thread runtime,
reserve reply capacity before FIFO interpretation, preserve continuation ownership, then exercise the
acceptance fixtures and record host measurement evidence. Other transports remain for M15b.

Follow [migration M15](../architecture/migration.md#m15-remote-adapters-on-commands-and-queries), including its tests, deletions, boundary rules and required unit round.
Keep the tracker brief. Record evidence here as the step progresses.

2026-11-18 gate-decision fix: record the owner's M15a acceptance and deferred M15b size criterion in ADR 0037
and the migration specification; collect the supplied unit and host measurements below, then run the fast gate.

## Measurements

Owner-reported results, 2026-11-18, for PR #145's measurement section. Unit results are verified on the DHO804;
host size measurements are after the gate fix. These are supplied results, not a new unit run for this docs fix.

| Measurement | Main | M15a (B) |
|---|---|---|
| Picture rate under a stalled client, unit | 0.2 fps | >= 26 fps |
| Remote throughput, unit | 30-34 exchanges/s | 628-1038 exchanges/s |
| Frame p50/p99, unit | <= 2.2/6.5 ms | <= 2.2/6.5 ms |
| CPU, wakeups, RSS, unit | Baseline | Within noise |
| Picture loss, unit | Baseline | <= 3 % |
| Production LOC, host | Baseline | +685 |
| aarch64 binary size, host | Baseline | +2.86 % |
| Dependencies, host | Baseline | +9 |

The owner accepts M15a now for the behaviour gain. Size is judged after M15b, once all transports use netio
and the old raw IO code is deleted. If the total then still adds production lines or binary size beyond noise,
replace tokio with a `poll(2)` IO thread keeping the same architecture. This supersedes the earlier M15a
size/rollout hold below; the deferred size gate is M15b's acceptance criterion.

## Status

- 2026-11-18: Recorded the owner's gate decision in ADR 0037, the M15a gate and M15b acceptance criterion,
  with the supplied unit and host measurements above for the coordinator's PR update. Docs only;
  no production change or new unit run. `cargo xtask check --fast` printed
  `partial gate passed (not done): 11 steps in 2 s` (exit 0); no ratchet counts fell.
  `git diff --check` passed. No crates changed, so no crate tests were needed.
  Git refresh/rebase, independent review and the PR body update remain with the coordinator.

- 2026-11-18: PR #145 check-fix evidence: the public run identifies `clippy (plans)`
  as the failed matrix job. With an empty Cargo cache, `cargo xtask step plan-matrix`
  printed `gate FAILED in 0 s: plan-matrix`: the receipted builder forced offline
  mode before dependencies were downloaded. Removing that forced flag retains
  `--locked` and explicit caller-selected offline mode. The cold-cache rerun prints
  `partial gate passed (not done): 1 steps`; all 65 register plans match unchanged.
  The regression fails before the fix and passes after; the 10 receipt tests and
  9 image-output tests pass. `OOS_PYTHON_NO_RUST=1 cargo xtask step python-tests`
  passes 333 tests, with 2 Cargo-dependent tests explicitly skipped.
  Clippy, decoder configurations and the fast gate also pass locally. Production
  and tooling LOC deltas are zero; no ratchet counts fell. Host only. Git fetch
  was sandbox-blocked; HEAD matches the existing remote-tracking ref. Git refresh,
  independent review and CI rerun remain with the coordinator.

- 2026-11-18: PR #145 check-fix plan: reproduce the plans job with an empty Cargo
  cache, allow its receipted build to download locked dependencies, add a cold-cache
  regression, and rerun the plans step, Python tests and fast gate. The empty-cache
  reproduction fails because the builder forces Cargo offline before dependencies exist.

- 2026-11-18: PR #145 correction: a held reply no longer requires every subsequent
  program to reserve a second full 64 MiB. Encoding uses the available reservation;
  per-connection output/credits and FIFO continuation ownership remain intact. The
  stalled-client regression fails with the old admission logic and passes with the fix.
  Raw SCPI accept/read/poll code was already absent; its remaining send helper now lives
  in `net` for VXI-11/HiSLIP only. No duplicate raw framing implementation remains.
  `:SYSTem:STATe?` adds `diag remote.enqueue` p50/p99 in nanoseconds, measured from
  complete-program acceptance before decoding to successful intake enqueue. It reports
  up to 256 observations within the last 10 seconds; fewer than 50 leaves p99 pending.
  Tokio features are `rt`, `net`, `time`, `sync`, `macros`; removing `io-util` removes
  `bytes`. Four `select!` calls retain macros for IO, ACL, shutdown and deadline races.
  Host baseline `origin/main` at `2e2af564f16b8647e3af61ba563d0d7eb35a67f2`:
  `cargo xtask loc --compare origin/main` reports UI 72,270 to 72,955 (+685),
  total 108,344 to 109,029 (+685), HAL unchanged. Same-target/profile aarch64
  release binaries: 6,966,688 to 7,165,928 bytes (+199,240, 2.86%). Normal
  dependency closure including local roots: 20 to 29; lockfile packages: 47 to 55.
  These comparisons supersede the earlier host counts for a different baseline.
  Full UI debug and test-release suites, including integration tests, pass; all 17
  perf-map tests pass. Clippy printed `partial gate passed (not done): 1 steps`;
  fast printed `partial gate passed (not done): 11 steps`. Initial size/import/fixture
  failures were corrected. Ratchet generation changed only the modified SCPI size entry.
  Logs and measurements are preserved under ignored `target/m15a-evidence/`.
  The shared `origin/main` ref advanced during the job to
  `46f1c8544efc2125f1925b1cdbc7a17ae7b22d3e` (CI-only); its production LOC
  baseline is unchanged. No Git writes or remote operations were performed here.
  Code and binary non-growth acceptance is still unmet. The coordinator owns Git
  refresh/rebase/publication, independent review and the new unit round; M15b stays held.

- 2026-11-18: PR #145 correction plan: admit replies against available global capacity so
  an IO-held reply cannot monopolise dispatch, keep connection credits through output, and
  test stalled/healthy clients with injected time. Remove unnecessary tokio features, audit
  replaced raw framing/IO, add accept-to-intake enqueue quantiles, then rerun the fast gate,
  UI tests and host LOC/dependency/binary measurements. Git updates and unit evidence remain
  with the coordinator. The rollout hold stays in place until the measurement gate passes.

- 2026-11-17: Host checks: `cargo xtask check --fast` printed
  `partial gate passed (not done): 11 steps`; clippy printed
  `partial gate passed (not done): 1 steps`. Cargo-deny advisories, bans, licences and sources passed.
  UI and xtask suites passed in debug and test-release profiles, including integration tests;
  final SCPI and IO regression reruns passed in both profiles. Earlier fmt, architecture, size
  and clippy failures were fixed. The ratchet retains reductions only for the changed SCPI file.
  Evidence and the draft PR body are in ignored build output under `target/m15a-evidence/`
  and `target/m15a-pr-body.md`; the coordinator must preserve these before retiring this worktree.
  Production LOC: UI 70,935 to 71,552 (+617). The aarch64 normal dependency closure is
  5 to 15 packages, including root/local crates; the all-target lockfile is 26 to 35 packages.
  The aarch64 release link passed: 6,872,992 to 7,073,464 bytes (+200,472, 2.9%).
  Unit measurements (frames, RSS, CPU, wakeups, latency and throughput) remain pending.

- 2026-11-17: M15a implementation on `refactor/138-m15a-netio`, based on
  `72dc61f8acad7dbc8687599eba60c351feb43245`. Raw socket IO removed from the frame thread;
  waveform encoding borrows samples and transfers response chunks. Cross-vendor review, integration
  and the unit measurement gate remain with the coordinator. M15b rollout is held pending that gate.

- 2026-11-14: seeded from task 0001. Not yet claimed.
