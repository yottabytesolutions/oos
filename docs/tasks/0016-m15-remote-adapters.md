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
`docs/adr/0023-io-layer.md`, `docs/architecture/migration.md`,
`docs/tasks/0016-m15-remote-adapters.md`.

M15a (#138): extend the owned intake seam, implement raw framing/output on one current-thread runtime,
reserve reply capacity before FIFO interpretation, preserve continuation ownership, then exercise the
acceptance fixtures and record host measurement evidence. Other transports remain for M15b.

Follow [migration M15](../architecture/migration.md#m15-remote-adapters-on-commands-and-queries), including its tests, deletions, boundary rules and required unit round.
Keep the tracker brief. Record evidence here as the step progresses.

2026-11-18 gate-decision fix: record the owner's M15a acceptance and deferred M15b size criterion in ADR 0023
and the migration specification; collect the supplied unit and host measurements below, then run the fast gate.

M15b HiSLIP (#151): retain framing, handshake, clear and acknowledgement behaviour behind owned requests.
Reuse netio admission, ACLs, output deadlines and cancellation; delete the frame-thread socket loops and
blocking send helper. Preserve fixtures, add injected-time stalled-output/cancellation coverage and measure
production LOC and same-profile aarch64 binary size. Additional reserved paths: `ui/src/hislip/`.

M15b HTTP/MCP (#151): replace accept/read/send threads and frame-owned sockets with netio
sessions and owned replies. Preserve the parser, DNS-rebinding checks, ACLs, status codes and
fixtures. Keep update bytes in a separately admitted worker, with only its bounded header and
job handle reaching intake. Reuse cancellation and output deadlines; test limits with injected time.
Additional reserved paths: `ui/src/http.rs`, `ui/src/http/`, `ui/src/netport/http/`,
`ui/src/netport/http.rs`, `ui/src/ui/update.rs`, `ui/src/ui/update/upload.rs`,
`ui/src/scpi/intake.rs`, `xtask/data/moves.txt` and affected generated ratchets.

HiSLIP switch (#163): the assigned brief authorizes extending this owning task. Add `remote.hislip`, default
off, to sysconf and the existing remote menu pattern; switch the scope listener, share its ACL, reload the
firewall and keep discovery tied to its actual listener. Preserve host serving. Test defaults, persistence,
menu layout, listener shutdown/restart and firewall rules, then run the fast gate and UI suites.
Additional reserved paths: `ui/src/system.rs`, `ui/src/sysconf/instruments.rs`, `ui/src/sysconf.rs`,
`ui/src/ui.rs`, `ui/src/ui/remote.rs`,
`ui/src/ui/menu.rs`, `ui/src/ui/menu/remote.rs`, `ui/src/ui/menu/ssh.rs`, `ui/src/ui/active_choice/system.rs`,
`rootfs/initramfs/bin/oos-firewall`,
`tools/tests/test_device_identity.py`, `ui/tests/remote-services.txt`, `ui/tests/golden/`,
`xtask/data/ui/size.txt`,
`docs/network.md`, `docs/adr/0019-instrument-protocols.md`, `docs/adr/README.md`, `CHANGELOG.md`.

M15b mDNS (#151): move discovery sockets, packet metadata and lifecycle onto the shared netio
runtime. Preserve responder fixtures, multicast membership, replies, goodbyes and interface refresh.
Delete replaced loops and unused network helpers; enforce the IO boundary with alias fixtures.
Repeat host stalled-client/cancellation/limit checks and report aggregate M15 LOC/aarch64 size.
Additional reserved paths: `ui/src/mdns.rs`, `ui/src/mdns/`, `ui/src/net6.rs`,
`ui/src/netacl.rs`, `ui/src/netport/http/`, `ui/src/main.rs`, `docs/adr/README.md` and affected generated ratchets.

PR #179 check fix: reserve `ui/src/ui/menu/tests.rs`; extract the renderer control assertions
to reduce the reproduced test complexity without changing coverage, then rerun clippy, UI tests and fast checks.

PR #179 simulation check fix: reserve `ui/src/ui/menu/simulation/tests.rs`; split setting-choice
and reference-slot coverage into separate tests and extract the row-inventory setup to satisfy the
line and complexity limits without changing assertions. Run clippy, UI tests in debug and
test-release, and the fast gate.

mDNS crate replacement (#194), owner-approved 2026-11-18: use the latest mdns-sd release,
retaining service names, TXT records, runtime switches and automatic IPv4/IPv6 address changes.
Replace the responder and socket glue; document removal of reverse PTR, QU unicast replies and
hostname publication without a registered service. Measure production LOC and matching ARM binaries,
exercise discovery and UI tests, run cargo-deny and the fast gate, and retain changed-file ratchets only.
Additional reserved paths: `docs/fork/deps.md`, `docs/network.md`, `CHANGELOG.md`.

PR #201 fast-check fix: refresh `fuzz/Cargo.lock` for mdns-sd while preserving existing package
versions. Remove the obsolete `fuzz/fuzz_targets/mdns.rs` parser target and its `fuzz/Cargo.toml`
registration because mdns-sd now owns packet parsing; update the fuzz list in `docs/development.md`.
Reproduce locked metadata and target compilation failures, then rerun those checks, discovery tests
in debug and test-release, and the fast gate. These additional paths are reserved for this fix.

PR #201 review fix: keep the existing claim on `refactor/194-mdns-crate`. Correct conflict
status and network transitions, bound daemon replies, retry failed discovery, and test real daemon
announcements and TXT wire bytes. Document fresh-daemon configuration and retain the shared lock
helper required by AGENTS.md. Run UI tests in debug and test-release and the fast gate.

PR #201 code-quality review fix: move address snapshots behind one event handler, name the
announcement formats, preserve typed discovery failures, and separate retry waits while retaining
Failed until a new attempt reports status. Move LAN interface enumeration to `netio/ifaddrs.rs`,
use typed TXT entries, and document the first-service announcement approximation. Extend existing
regressions, run UI tests in debug and test-release, tighten changed-file ratchets, and run fast checks.

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

The owner accepts M15a for the behaviour gain. The IO runtime decision of 2026-11-18 chose tokio for all
network IO and retires the deferred size gate and `poll(2)` fallback. Each transport still reports and
minimises its production LOC and aarch64 binary size delta.

### M15b closure measurements, host only

Measured 2026-11-18. Baseline is the pre-M15a main at
`2e2af564f16b8647e3af61ba563d0d7eb35a67f2`. The M15-only replay applies M15a and all four M15b
transport slices, retaining the baseline's update identity API names and ACL implementation to exclude
intervening boot/model and menu changes. Counts use the unchanged xtask production measurement; tests,
comments and tooling are excluded. Both default-feature ARM builds use the pinned toolchain,
`--locked --release --target aarch64-unknown-linux-musl`, with the same release profile and CPU flags.
Both ELF files pass `cargo xtask elf-static`.

| Aggregate M15 measurement | Pre-M15a | M15-only | Delta |
|---|---:|---:|---:|
| UI production LOC | 72,270 | 73,492 | +1,222 |
| Total production LOC | 108,344 | 109,566 | +1,222 |
| aarch64 release ELF bytes | 6,966,704 | 7,252,104 | +285,400 (+4.10 %) |

The mDNS slice itself changes UI/total production LOC from 73,491/109,799 to 73,486/109,794 (-5),
with +8 xtask tooling lines and no new dependencies. Cumulative growth pays for shared bounded
intake/reply ownership, cancellation and protocol framing; the earlier negative LOC expectation is unmet.
These are reporting measurements, not a size gate (ADR 0023).

Host `--serve`, dev profile, demo acquisition on channels 1 and 2: medians of three paired 1.5 s windows
after 100 warm-up queries. The stalled peer leaves a 1,843,254-byte BMP reply unread with a 4 KiB
receive buffer; a non-consuming header peek verifies the large reply before measurement. Enqueue p99
uses the existing rolling 256-observation diagnostic. These host figures do not substitute for unit results.

| Host measurement | Healthy client | Beside stalled client |
|---|---:|---:|
| Exchanges/s | 2,597.3 | 2,848.6 |
| Pictures/s | 36.76 | 36.67 |
| Round-trip p50/p99, ms | 0.088/16.845 | 0.095/16.661 |
| Server enqueue p99, microseconds | 6.750 | 6.708 |

All three stalled-client runs answer after disconnect. Eight raw clients remain admitted, the ninth is
closed, and disconnect releases a slot for re-admission. Injected-time fixtures retain the 2 s stalled and
30 s total output deadlines, release credits/reservations on cancellation and preserve bounded request,
reply, snapshot and upload admission. The explicit loopback multicast responder test passes.

## Status

- 2026-11-18: PR #201 conflict-label and unsafe-placement fixes reserve the full u32
  conflict suffix budget at registration and move the unchanged LAN walk into HAL system.
  The maximum-label regression fails before the fix (exit 101); all three new regressions
  pass afterward. The pointer warning is a false positive: successful allocation, null-checked
  links and traversal before the single free preserve validity. A thread reply records this evidence.
  `cargo test --locked -p oos-hal -p oos-ui` and the same command with `--profile test-release`
  pass (exit 0), including integrations, goldens and allocation soaks. HAL has 408 passing unit
  tests and 8 existing skips; UI has 1623/1622 passing unit tests and 58 existing skips per profile.
  Initial fast printed `gate FAILED in 16 s: unsafe` (exit 1). Generated ratchet tightening
  removes only the obsolete netio unsafe entry; the two unsafe moves are also removed.
  Fast then prints `partial gate passed (not done): 12 steps in 52 s`; the CI-base invocation
  prints `partial gate passed (not done): 12 steps in 43 s` (both exit 0).
  Added-line privacy and diff checks pass. Production LOC increases by 7 (HAL +32, UI -25);
  tooling is unchanged. Host only. Git synchronization, independent review and final CI remain
  with the coordinator under the assigned restrictions.

- 2026-11-18: PR #201 conflict-label and unsafe-placement review fix plan: reserve mDNS
  host/instance suffix space at registration, retaining configured names and UTF-8 boundaries.
  Move LAN enumeration into `oos_hal::system`, preserving its filtering and list lifetime;
  assess the pointer warning against the successful `getifaddrs` ownership contract.
  Additional reserved paths: `hal/src/system.rs`, `hal/src/system/ifaddrs.rs` and
  `ui/src/ui/remote.rs`. Reuse discovery and ACL regressions, run HAL/UI debug and
  test-release suites, generate changed-file ratchet reductions and run the fast gate.
  The runner retains Git synchronization, review and integration.

- 2026-11-18: PR #201 fast-check fix restores both unsafe moves, preserving the intermediate
  socket path required by the current CI base. Before the fix, `CI=1 OOS_GATE_BASE=origin/main
  cargo xtask check --fast` printed `gate FAILED in 36 s: unsafe` (exit 1), rejecting
  `unsafe ui/src/netio/ifaddrs.rs` as a new ratchet entry. After the fix, the default fast gate
  prints `partial gate passed (not done): 12 steps in 29 s`, and the same CI-base invocation
  prints `partial gate passed (not done): 12 steps in 30 s` (both exit 0).
  Complete xtask suites pass in debug and test-release: 242 tests each. Focused UI discovery
  tests pass in both profiles: 25 passed and 1 existing live multicast skip per binary.
  Locked fuzz metadata and host shell checks pass; `git diff --check` passes. Production
  and tooling LOC deltas are zero; no ratchet counts changed or allowances increased.
  Host only. No remote Git operations under the assigned restrictions; the coordinator retains
  synchronization, independent review and the required CI rerun.

- 2026-11-18: PR #201 fast-check fix plan: preserve the unsafe move chain from `mdns.rs`
  through `netio/mdns/socket.rs` to `netio/ifaddrs.rs` in `xtask/data/moves.txt`.
  The default local base passes, but `CI=1 OOS_GATE_BASE=origin/main` rejects the destination
  as a new unsafe allowance because the intermediate move is missing. Recheck both bases,
  locked fuzz metadata, host shell scripts and discovery tests in debug and test-release.
  No production or runtime behaviour changes; Git updates remain with the coordinator.

- 2026-11-18: PR #201 code-quality fixes put address snapshot refresh behind `on_event`,
  extract announcement/host-label decoding, preserve typed daemon/timeout/closed/stopped errors,
  and keep Failed across retry backoff. LAN enumeration now lives in `netio/ifaddrs.rs`, with
  direct policy/menu callers and a generated unsafe ratchet move. TXT entries carry typed optional
  values; Running explicitly means at least one service announced. Focused discovery tests pass
  (23 passed, 1 live multicast test skipped), and added lines have zero privacy findings.
  Full UI suites pass: debug 1663 tests, test-release 1662 tests, with 59 existing skips each
  (exit 0). All-target UI Clippy exits 0; warnings in changed files match existing ratchet entries.
  Fast prints `partial gate passed (not done): 12 steps in 47 s` (exit 0). No changed-file
  ratchet counts fell; `cargo xtask ratchet --move` generated the unsafe relocation, retaining
  unrelated move entries. Firmware production LOC increases by 56 versus the pre-fix commit;
  tooling is unchanged. Host only. No fetch/reset under the assigned network/Git-write restrictions;
  HEAD matches the cached remote-tracking branch. Independent review and integration remain with
  the coordinator.

- 2026-11-18: PR #201 review fixes centralize active discovery state, reflect instance conflicts,
  strip one local suffix, bound daemon replies and shutdown to 500 ms, and retry errors after 2 s.
  Configuration changes cancel pending status replies; configuration applies once per fresh daemon.
  Real loopback daemon events and TXT wire bytes supplement virtual-time retry/timeout regressions.
  The shared poison-tolerant lock helper remains as required by AGENTS.md. Fast prints
  `partial gate passed (not done): 12 steps in 8 s` (exit 0); changed-file ratchets did not fall.
  Firmware production LOC increases by 49; tooling is unchanged. Full UI debug and test-release suites
  pass (exit 0), including integration and allocation tests. UI all-target Clippy exits 0 with no
  findings in changed mDNS files. Added lines have zero privacy findings. Host only; fetch is
  sandbox-blocked and HEAD matches the cached remote-tracking branch. Git refresh, independent review
  and integration remain with the coordinator.

- 2026-11-18: PR #201 fast-check failure reproduced locally: locked fuzz metadata rejected
  the missing mdns-sd dependency graph. Regenerated the fuzz lockfile with all existing package
  versions preserved. All remaining fuzz targets compile after removing the obsolete parser target;
  the original target failed with E0425 for the removed `mdns::parse`. Locked metadata and all
  39 host-script shellchecks pass. Focused debug and test-release discovery tests pass (7 passed,
  1 live multicast test ignored per UI target/profile). Fast prints
  `partial gate passed (not done): 12 steps in 22 s` (exit 0); `git diff --check` passes.
  Firmware production LOC is unchanged, and no changed-file ratchets fell. Host only; fetch was
  sandbox-blocked, and HEAD matches the cached remote-tracking branch. The coordinator retains
  Git refresh, independent review and CI publication.

- 2026-11-18: #194 privacy retry keeps the completed replacement. Synthetic serial fixtures now use
  concatenated parts with identical values and assertions. Working-tree additions and the commit message
  have zero privacy findings; the whole-tree audit still reports existing findings outside the additions.
  UI debug and test-release suites, including integration and allocation plateau tests, passed (exit 0).
  Fast gate passed all 12 selected steps (exit 0); production LOC and ARM binary measurements are unchanged.

- 2026-11-18: #194 replaced the responder and packet/socket implementation with mdns-sd 0.22.0.
  All six service definitions and switches remain; address refresh is automatic every 2 s.
  The three approved removals are documented in network.md and the daemon exception in ADR 0023.
  UI/total production LOC: 72,830/109,372 to 71,632/108,174 (-1,198); tooling unchanged.
  Matching default-feature aarch64 musl release ELF: 7,368,984 to 7,780,824 bytes
  (+411,840, +5.59%). Cargo.lock packages: 56 to 67 (+11).
  Host UI suites passed in debug and test-release, including integration tests. Focused discovery
  passed (7 tests, 1 opt-in test ignored); the opt-in loopback test then passed for all six
  service types, individual withdrawal, restart and mDNS disable. Cargo-deny: advisories ok, bans ok, licenses ok, sources ok
  against the cached advisory database. Clippy: `partial gate passed (not done): 1 steps in 23 s`.
  Fast gate: `partial gate passed (not done): 12 steps in 8 s` (exit 0), after fixing the
  initial size/unsafe failures with generated ratchet reductions for the replaced files only.
  No hardware run. Independent review, CI and publication remain with the coordinator.

- 2026-11-18: #194 implementation assigned on `refactor/194-mdns-crate`, based on
  `a8eb80da3ca7e4974f3e1e085588b5fc77a7d4a9`.
  Baseline UI/total production LOC is 72,830/109,372. The coordinator owns Git writes,
  independent review, CI and publication. No hardware access is assigned.

- 2026-11-18: PR #173 follow-up check fix: extracted simulation fixture setup and
  separated decorated-choice and reference-slot assertions into independent tests.
  Host and ARM Clippy passed in 222 s with no ratchet reductions. The fast gate printed
  `partial gate passed (not done): 11 steps in 14 s` (exit 0).
  Complete `cargo test --locked -p oos-ui` and
  `cargo test --locked --profile test-release -p oos-ui` suites passed (exit 0):
  1,577 debug and 1,576 release unit tests plus 43 integration tests per profile,
  with 59 existing ignores each. Production LOC delta is zero. Host only; no unit access.
  All 65 register plans and all 15 standalone decoder configurations passed:
  `partial gate passed (not done): 3 steps in 419 s` (exit 0).
  Fetch was blocked by sandbox permissions; synchronization, independent review and final
  CI remain with the coordinator.

- 2026-11-18: PR #173 follow-up check-fix plan: reserve
  `ui/src/ui/menu/simulation/tests.rs` for test-only fixture extraction and choice-test
  separation, retaining all assertions. Local Clippy printed `gate FAILED in 39 s: clippy`
  (exit 1): the row test is 65/60 lines and the choice test has cognitive complexity 28/25.
  Verify Clippy, decoder configurations, register plans, the fast gate and complete UI
  debug/test-release suites. Fetch was blocked by sandbox permissions; cached branch refs
  match. Remote synchronization and final CI remain with the coordinator.

- 2026-11-18: PR #173 check fix: extracted the row-control assertions into a test helper,
  preserving every menu, text-size, picker and navigation case. Final Clippy printed
  `partial gate passed (not done): 1 steps in 69 s` (exit 0); no ratchet reductions were reported.
  The fast gate printed `partial gate passed (not done): 11 steps in 17 s` (exit 0).
  Complete `cargo test --locked -p oos-ui` and
  `cargo test --locked --profile test-release -p oos-ui` suites passed (exit 0):
  1,556 debug and 1,555 release unit tests plus 43 integration tests per profile,
  with 59 existing ignores each. The extracted test also passed a final exact debug run.
  `cargo xtask loc --compare HEAD` reports zero production-line change.
  Logs remain in ignored `target/pr173-*.log`. Host only; no hardware access or Git writes.
  Remote synchronization, independent review and final CI remain with the coordinator.

- 2026-11-18: PR #173 check-fix plan: reserve `ui/src/ui/menu/tests.rs` to extract the
  row-control assertions from the renderer/picker/navigation test without changing its cases.
  Local `cargo xtask step clippy` reproduced `gate FAILED in 61 s: clippy` (exit 1):
  the test's cognitive complexity is 26/25. Run Clippy, the fast gate and complete UI
  debug/test-release suites; retain only generated ratchet reductions for changed files.
  Git updates and remote checks remain with the coordinator.

- 2026-11-18: PR #179 simulation check fix: extracted the row-inventory setup and separated
  reference-slot choices from setting choices, preserving all assertions and production behaviour.
  Clippy passes with `partial gate passed (not done): 1 steps in 107 s` (exit 0).
  Initial clippy printed `gate FAILED in 2 s: clippy`; after splitting choices it printed
  `gate FAILED in 96 s: clippy` until the separate row-inventory setup was extracted.
  `cargo test --locked -p oos-ui` and its `--profile test-release` counterpart pass (exit 0):
  1,577 debug / 1,576 release unit tests and 43 integration tests per profile, with 58 unit
  and one golden existing ignores. Fast passed with
  `partial gate passed (not done): 11 steps in 14 s` (exit 0).
  Production LOC delta is zero; no changed-file ratchet reductions were reported.
  Host only. Fetch was blocked by sandbox Git metadata permissions; remote synchronization,
  independent review and CI remain with the coordinator.

- 2026-11-18: PR #179 simulation check-fix plan: local clippy reproduces
  `gate FAILED in 2 s: clippy` on simulation tests (row inventory 65/60 lines, choices complexity 28/25).
  Extract row setup and split the independent reference-slot cases while preserving every assertion. Host only;
  Git synchronization and CI remain with the coordinator because the sandbox blocks fetch metadata writes.

- 2026-11-18: PR #179 review fix: discovery receives consume Tokio's cooperative budget.
  The queued-packet regression fails before the fix (exit 101) and passes after for IPv4 and IPv6.
  Both `getifaddrs` pointer findings are false positives: successful allocation, non-null list links
  and traversal before `freeifaddrs` preserve pointer validity. No interface code changed.
  `cargo test --locked -p oos-ui` and its `--profile test-release` counterpart pass (exit 0):
  1,576 debug / 1,575 release unit tests and 43 integration tests per profile; existing ignores remain.
  Fast prints `partial gate passed (not done): 11 steps in 5 s` (exit 0).
  Clippy prints `gate FAILED in 55 s: clippy` (exit 1) on unchanged
  `ui/src/ui/menu/simulation/tests.rs`: 65/60 lines and complexity 28/25.
  Production LOC increases by one; no changed-file ratchet reductions were reported.
  Host only. Git synchronization, unrelated clippy findings, independent review and CI remain with the coordinator.

- 2026-11-18: PR #179 review-fix plan: budget discovery receives on the shared executor and add a
  deterministic queued-packet regression for both address families in `ui/src/netio/mdns/socket/`.
  Check the two interface-list pointer findings against the `getifaddrs` lifetime contract;
  run UI tests in debug and test-release and the fast gate. Git and CI remain with the coordinator.

- 2026-11-18: PR #179 check fix: `cargo xtask step clippy` reproduced
  `gate FAILED in 56 s: clippy` on the menu-renderer test's cognitive complexity (26/25).
  Extracted its row-control assertions into a test helper, preserving all checks and production behaviour.
  Clippy now prints `partial gate passed (not done): 1 steps in 153 s` (exit 0);
  fast prints `partial gate passed (not done): 11 steps in 18 s` (exit 0).
  `cargo test --locked -p oos-ui` and its `--profile test-release` counterpart pass (exit 0):
  1,562 debug / 1,561 release unit tests and 43 integration tests per profile,
  with 58 unit and one golden existing ignores per profile. Production LOC delta is zero;
  no ratchet reductions were reported. Host only. Remote refresh, review and CI remain with the coordinator.

- 2026-11-18: Final host validation passes (exit 0): `cargo test --locked -p oos-ui -p xtask`
  and its `--profile test-release` counterpart, including 1,547 debug / 1,546 release UI unit
  tests, 43 integration tests per profile and 238 xtask tests per profile. Existing ignores:
  58 unit tests and one golden test per profile. The explicit loopback multicast test and all
  host load/connection-limit trials pass. `cargo xtask step clippy` prints
  `partial gate passed (not done): 1 steps in 183 s` (exit 0); the fast gate prints
  `partial gate passed (not done): 11 steps in 13 s` (exit 0).
  Initial fast printed `gate FAILED in 3 s: size, architecture, unsafe`: generated reductions
  and filtering test-only imports resolve those findings. Clippy initially printed
  `gate FAILED in 55 s: clippy`; moving the discovery export before the test module resolves
  `items_after_test_module`. `cargo xtask ratchet --move` and `cargo xtask ratchet` passed;
  only changed mDNS size/lint/unsafe entries are retained, including the unsafe move (8 to 7).
  Complete logs, measurement scripts, source snapshots and ELF files remain in ignored
  `target/m15b-mdns/`. Git changes, independent review, CI and integration remain with the
  coordinator. No unit run or hardware verification is claimed.

- 2026-11-18: M15b mDNS (#151): discovery uses the same netio executor as the instrument and web
  transports. The packet responder, records, goodbyes and IPv4/IPv6 metadata remain; the dedicated
  thread, control wake and poll loop are deleted. Net6 binding helpers now live in netio; the gate
  confines std::net with alias/re-export fixtures and explicit existing worker exceptions in ADR 0023.
  The slice removes 5 production lines and adds 8 tooling lines. The full M15 task remains active
  for the parser work; the M15b transport cutover is implemented and host-verified. No hardware access.

- 2026-11-18: M15b mDNS plan accepted through the assigned brief on `refactor/151-m15b-mdns`. Reuse the existing task
  and runtime; preserve protocol behaviour and exercise host discovery/lifecycle and shared limits.
  The coordinator owns Git, independent review, final CI and integration. Host only.


- 2026-11-18: PR #168 screenshot review fix: 200 and 304 replies share the bounded error
  fallback; an exhausted output budget closes the reply and wakes IO. The matching-ETag
  long-poll regression fails before the fix (exit 101) and passes after with injected time.
  Final `cargo test --locked -p oos-ui` and
  `cargo test --locked --profile test-release -p oos-ui` passed (exit 0): 1,545 debug and
  1,544 release unit tests plus 43 integration tests per profile; 59 existing ignores.
  An earlier release attempt failed `warmed_ui_observer_allocation_census` with three
  finish-capture allocations; it passed in both final suites. The initial fast gate printed
  `gate FAILED in 8 s: size`; sharing the response path resolved the growth, and the gate
  printed `partial gate passed (not done): 11 steps in 14 s` (exit 0).
  `cargo xtask ratchet` passed and changed only `ui/src/http.rs`'s size entry, 660 to 657.
  Production LOC decreased by three. Complete logs remain in ignored `target/http-304-*.log`.
  Host only; no hardware access or Git writes. Branch synchronization, independent review
  and integrated checks remain with the coordinator.

- 2026-11-18: PR #168 screenshot review-fix plan: use the bounded error fallback when a
  matching-ETag 304 response cannot reserve output bytes. Reserve `ui/src/http.rs` for the fix and
  a deterministic long-poll regression with an exhausted output budget. Run UI tests
  in debug and test-release and the fast gate; Git updates remain with the coordinator.

- 2026-11-18: PR #168 review fixes: complete HTTP requests survive request-side EOF while
  socket errors still cancel waiting work. An error response that cannot reserve byte capacity
  closes its reply and wakes IO. Both regressions fail before the fixes (exit 101) and pass after.
  `cargo test --locked -p oos-ui` and `cargo test --locked --profile test-release -p oos-ui`
  passed (exit 0): 1,544 debug and 1,543 release unit tests, plus 43 integration tests in each
  profile; 59 existing ignores per profile. `cargo xtask check --fast` printed
  `partial gate passed (not done): 11 steps in 3 s` (exit 0); `cargo xtask step clippy` printed
  `partial gate passed (not done): 1 steps in 43 s` (exit 0). No changed-file ratchets decreased.
  `cargo xtask loc --compare HEAD` reports UI 73,488 to 73,494 and total 109,796 to 109,802
  (+6 production lines). Complete logs remain in ignored `target/http-*.log`.
  Host only; no hardware access or Git writes. Remote synchronization, independent review,
  the integrated full gate and integration remain with the coordinator.

- 2026-11-18: PR #168 review-fix plan: preserve complete HTTP requests across write-side EOF,
  close replies when their error response cannot reserve byte capacity, and add deterministic
  socket and exhausted-budget regressions. Reserved paths remain `ui/src/netio/http.rs`,
  `ui/src/netio/http/tests.rs` and `ui/src/netport/http.rs`. Run the fast gate and UI tests
  in debug and test-release; the coordinator owns Git updates and integration.

- 2026-11-18: M15b HTTP/MCP (#151), branch `refactor/151-m15b-http`: HTTP framing and all socket writes
  now share netio with SCPI, VXI-11 and HiSLIP. Static pages bypass frame intake; live pictures retain their
  encoder and use owned reply handles. Single-large-body admission and the signed update worker remain;
  only a bounded header and job handle reach frame intake. Legacy accept/read/send loops are deleted.
  Existing HTTP/MCP, Host/Origin, ACL, status and export fixtures pass. Injected time covers read idle/total,
  intake expiry, partial sends and both output deadlines; cancellation covers body admission, queued intake,
  snapshots and joined shutdown. Production delta is +316 lines (shared output chunks, reply ownership and
  upload-worker handles); no hardware was accessed. Matching `--release` aarch64 ELF builds measured
  7,214,632 -> 7,262,192 bytes: +47,560 (+0.66%). Approved tokio `io-util` adds the `bytes` transitive
  dependency and replaces manual async read/write plumbing. `cargo test --locked -p oos-ui` passed
  1,534 unit tests and 43 integration tests; `cargo test --locked --release -p oos-ui` passed 1,533 unit
  tests and the same 43 integration tests (59 existing ignores in each profile). `cargo xtask step clippy`
  printed `partial gate passed (not done): 1 steps in 1 s`. `cargo xtask check --fast` printed
  `partial gate passed (not done): 11 steps in 2 s`.
  Independent review, the full gate, new unit/load measurements and integration remain with the coordinator.
  Earlier checks printed `gate FAILED in 10 s: size, mutable-statics`, `gate FAILED in 4 s: architecture`
  and `gate FAILED in 29 s: clippy`; factoring input, recording the sender-counter move, retaining the
  port boundary and deleting the obsolete synchronous helper resolved those findings.

- 2026-11-18: HiSLIP switch (#163) on `feat/163-hislip-switch`, base
  `ff535fb732c18cc79d12b428f236edc5d857a311`: implemented `remote.hislip`, default off, with sysconf persistence,
  remote menu/active choice, scope listener switching, shared ACL, firewall reload and discovery lifecycle.
  Host serving retains HiSLIP. No peer remote-service switch has an SCPI command, so none was added.
  Grouped instrument flags and moved menu metadata/anchoring to meet existing size limits without loosening
  ratchets. Initial fast checks failed on size and a rejected test-module path attribute; both were corrected.
  Initial Clippy rejected the long lifecycle test; its setup is now a helper. An initial persistence test
  compared image-only state with serialized state; it now reloads over the same image defaults.
  Python firewall/identity tests pass (11); explicit golden update passes all 269 cases, 538 fresh processes
  and 1266 repeat captures, adding only six remote-service captures and their manifest entries. New off/on
  menu images were visually checked at normal and larger text sizes. Generated size ratchets retain only
  reductions for the changed UI and remote-menu files. Final complete UI suites pass (exit 0): 1,539 debug
  and 1,538 test-release unit tests, with 58 ignored in each. Both profiles pass 6 golden tests (1 ignored),
  15 host-script, 8 host-socket and 14 soak tests. Clippy prints
  `partial gate passed (not done): 1 steps in 102 s` (exit 0). Fast prints
  `partial gate passed (not done): 11 steps in 14 s` (exit 0); `git diff --check` passes.
  Earlier checks printed `gate FAILED in 3 s: size`, `gate FAILED in 5 s: size`, and
  `gate FAILED in 2 s: size, architecture, mutable-statics, unsafe, method-exceptions, known-failures, test-time`
  (the forbidden module-path attribute); Clippy printed `gate FAILED in 85 s: clippy` before the test split.
  `cargo xtask loc --compare HEAD` reports UI 73,172 to 73,231 (+59 production Rust lines); shell tooling
  grows by 4 firewall lines. Other production crates and dependencies are unchanged.
  Host only; no hardware or Git writes. Coordinator owns independent review, full integration gate and unit verification.

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

- 2026-11-18: M15b HiSLIP (#151) plan accepted through the assigned brief: amend ADR 0023 and M15a/M15b,
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
  under ADR 0023. Host only; independent review, final CI and unit load measurements remain
  with the coordinator. No Git writes or remote operations were performed.

- 2026-11-18: M15b VXI-11 (#151) plan: reuse netio runtime, bounded request/reply ports,
  cancellation and timed output for portmapper TCP/UDP and core TCP. Preserve frame-owned
  links, RPC fixtures, program continuations and all protocol limits. Delete replaced socket
  polling, exercise injected-time output and host RPC fixtures, then measure production LOC
  and same-profile aarch64 binary size. No abort channel exists. Coordinator owns review,
  integration and unit measurements. Additional reserved paths: `ui/src/vxi11/`,
  `ui/src/system/serve.rs`, `ui/src/scpi/intake.rs`, `docs/architecture/system.md`.

- 2026-11-18: Recorded the owner's gate decision in ADR 0023, the M15a gate and M15b acceptance criterion,
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
  Production LOC: UI 70,935 to 71,552 (+617). The aarch64 normal dependency closure is
  5 to 15 packages, including root/local crates; the all-target lockfile is 26 to 35 packages.
  The aarch64 release link passed: 6,872,992 to 7,073,464 bytes (+200,472, 2.9%).
  Unit measurements (frames, RSS, CPU, wakeups, latency and throughput) remain pending.

- 2026-11-17: M15a implementation on `refactor/138-m15a-netio`, based on
  `72dc61f8acad7dbc8687599eba60c351feb43245`. Raw socket IO removed from the frame thread;
  waveform encoding borrows samples and transfers response chunks. Cross-vendor review, integration
  and the unit measurement gate remain with the coordinator. M15b rollout is held pending that gate.

- 2026-11-14: seeded from task 0001. Not yet claimed.
