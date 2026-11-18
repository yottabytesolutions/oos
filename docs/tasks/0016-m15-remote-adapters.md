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

M15b HiSLIP (#151): retain framing, handshake, clear and acknowledgement behaviour behind owned requests.
Reuse netio admission, ACLs, output deadlines and cancellation; delete the frame-thread socket loops and
blocking send helper. Preserve fixtures, add injected-time stalled-output/cancellation coverage and measure
production LOC and same-profile aarch64 binary size. Additional reserved paths: `ui/src/hislip/`.

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

The owner accepts M15a for the behaviour gain. ADR 0037 amendment 2026-11-18 (2) chooses tokio for all
network IO and retires the deferred size gate and `poll(2)` fallback. Each transport still reports and
minimises its production LOC and aarch64 binary size delta.

## Status

- 2026-11-18: M15b HiSLIP (#151) implementation: sync/async channels use the shared current-thread runtime,
  request admission, connection credits, ACL rechecks, disconnect cancellation and timed output. Protocol
  sessions and SCPI continuations stay frame-owned. Borrowed payload slices and stack headers replace the
  old per-fragment wire copies; the frame-thread socket loops and last blocking send helper are deleted.
  Async clear no longer carries unreachable per-async-channel program/payload clearing. Existing handshake,
  acknowledgement-only locking, switches, limits, error codes and protocol fixtures remain.
  Added injected-time intake/output checks plus fragmentation, malformed/oversized headers, read-ahead
  overflow, eight-peer fairness, backpressure, disconnect and cancellation regressions. The first fast gate
  printed `gate FAILED in 2 s: size`; shared reply pumping corrected the oversized polling function.
  The first full release suite found a clear/read-ahead race that discarded a fresh post-acknowledgement
  command. Clearing buffered bytes before the next socket read fixes it. Its deterministic regression
  fails with that fix removed (exit 101) and passes with it restored; all 16 focused HiSLIP tests pass in
  debug and test-release. Final complete UI suites pass (exit 0): 1,511 debug and 1,510 release unit tests,
  56 ignored in each; both profiles pass 6 golden tests (1 ignored), 15 host-script tests, 8 host-socket
  tests and all 14 soak tests. Fast printed `partial gate passed (not done): 11 steps in 4 s` (exit 0);
  clippy printed `partial gate passed (not done): 1 steps in 83 s` (exit 0). Generated ratchets retain only
  the reduced HiSLIP file/function size and lint entries. `git diff --check` passes.
  `cargo xtask loc --compare HEAD`, base `e846b2cf7080ce93d2713c18e644e3803a4289a1`, reports UI 73,100
  to 73,199 (+99), total 109,145 to 109,244 (+99); tooling and dependencies are unchanged. Separate
  baseline/candidate default-feature aarch64 musl release builds report 7,197,912 to 7,206,312 bytes
  (+8,400, +0.12%). The net reduction aim is unmet: bounded continuation/reply ownership, cancellable
  clear read-ahead and shared streaming framing add more lines than the deleted socket loops remove.
  Complete logs, baseline source/binary and measurements remain in ignored `target/m15b-hislip/`.
  Host only; no hardware access or Git writes. Independent review, the integrated full gate and unit load
  measurements remain with the coordinator; the wider M15 task stays active.

- 2026-11-18: M15b HiSLIP (#151) plan accepted through the assigned brief: amend ADR 0037 and M15a/M15b,
  migrate sync/async channels to netio on `refactor/151-m15b-hislip`, preserve protocol fixtures,
  then run fast/debug/release checks and
  measure LOC/binary deltas. Base: `e846b2cf7080ce93d2713c18e644e3803a4289a1`. Coordinator owns Git,
  independent review and integration; host only.

- 2026-11-18: PR #155 size-fix result: shared service switching/runtime retention/ACL
  publication and parked-reply retry replace duplicated raw/VXI plumbing. Link ownership
  is assigned when creating a link, removing serialized-reply parsing; shared XDR word
  output preserves RPC errors and payloads. Existing VXI tests pass (17/17). Production
  LOC against `origin/main`: UI 72,976 to 73,100 (+124), total 109,021 to 109,145 (+124),
  tooling unchanged. This removes 36 production lines from the submitted +160, but does
  not meet the <= 0 acceptance criterion. Tests and docs were already excluded correctly.
  Remaining growth pays for the shared executor lifetime, separate UDP admission alongside
  eight TCP slots, disconnect/link cancellation, and frame-owned bounded pending/reply
  dispatch. These cannot simply be deleted without removing retained transport guarantees;
  further structural replacement remains an acceptance decision for the coordinator.
  The final fast gate printed `partial gate passed (not done): 11 steps in 6 s` (exit 0).
  Clippy printed `partial gate passed (not done): 1 steps in 172 s` (exit 0).
  `cargo test --locked -p oos-ui` and the same command with `--profile test-release`
  both exit 0, including golden, host-script/socket and soak suites. Unit suites pass
  1,502 debug and 1,501 release tests, with 56 ignored in each; all 14 soak tests pass
  in each profile. Generated ratchet changes are limited to the modified SCPI file size
  and VXI core function size. `git diff --check` passes. Complete logs and production
  per-file token/diff accounting remain in ignored `target/m15b-size-audit/`.
  Binary size was not remeasured. Host only; no hardware access or Git writes.

- 2026-11-18: PR #155 size-fix plan: account for production additions/removals against
  `origin/main`, share raw/VXI service lifecycle, ACL propagation and parked-reply delivery,
  and simplify RPC framing/dispatch only where existing behaviour is preserved. Retain
  RPC fixtures, limits and error codes. Run the fast gate, UI debug/release tests and
  production LOC measurement. The runner owns Git updates; fetch is sandbox-blocked.

- 2026-11-18: M15b VXI-11 (#151) implementation: TCP/UDP portmapper and core channel
  share raw SCPI's current-thread runtime and owned request/reply ports. Frame-owned link
  and pending-program handling retains RPC fixtures, error codes and text/binary encoding;
  eight TCP slots remain available beside UDP admission. Replaced VXI-11 accept/read/send/poll
  code and its unused shared socket-read helper are deleted. No abort channel existed.
  No Tokio features or dependencies were added. Host debug and test-release UI suites pass,
  including golden, host script/socket and soak suites; focused RPC/IO regressions cover
  fragmented/malformed/oversized records, binary/empty/large reads, pending-program/link
  limits, injected-time backpressure/deadlines, cancellation and independent service shutdown.
  Clippy printed `partial gate passed (not done): 1 steps in 30 s` (exit 0).
  Fast printed `partial gate passed (not done): 11 steps in 2 s` (exit 0).
  `cargo xtask ratchet` retains reductions only for modified SCPI/VXI-11 size/lint entries
  and the deleted `net` unsafe block. `git diff --check` passes.
  `cargo xtask loc --compare HEAD`, baseline `e088534a524785bba3c38a77acd4881eb10e668d`,
  reports UI 72,955 to 73,115 (+160), total 109,029 to 109,189 (+160); tooling unchanged.
  Same default-feature aarch64 musl release builds: 7,165,960 to 7,200,224 bytes
  (+34,264, +0.48%). The candidate UI package was rebuilt after clearing its release artifacts
  to avoid reusing the archived baseline's shared-target fingerprint. Logs, source/binary
  measurements and the baseline executable remain in ignored `target/m15b-evidence/`.
  The expected negative LOC delta is unmet. Aggregate M15b size acceptance remains open
  under ADR 0037. Host only; independent review, final CI and unit load measurements remain
  with the coordinator. No Git writes or remote operations were performed.

- 2026-11-18: M15b VXI-11 (#151) plan: reuse netio runtime, bounded request/reply ports,
  cancellation and timed output for portmapper TCP/UDP and core TCP. Preserve frame-owned
  links, RPC fixtures, program continuations and all protocol limits. Delete replaced socket
  polling, exercise injected-time output and host RPC fixtures, then measure production LOC
  and same-profile aarch64 binary size. No abort channel exists. Coordinator owns review,
  integration and unit measurements. Additional reserved paths: `ui/src/vxi11/`,
  `ui/src/system/serve.rs`, `ui/src/scpi/intake.rs`, `docs/architecture/system.md`.

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
