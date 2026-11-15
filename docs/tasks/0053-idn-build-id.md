---
id: 53
title: "Report the build in *IDN?"
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

# Report the build in *IDN?

## Goal

`*IDN?` names the running build, so remote tools and unit rounds can tell builds apart.

## Plan

Reserved paths: `ui/src/ui/host.rs`, `ui/src/ui/sysinfo.rs`, `ui/src/scpi.rs`, `ui/src/http.rs`,
`docs/tasks/0053-idn-build-id.md`, `target/COMMIT_MSG.txt` (worktree-local).

1. Cache the firmware identity when constructing the system Host, reading `/etc/oos-build`
   through its supplied text reader. The Host owns it for the process lifetime, bounded
   by the package version plus a 128-byte build token and fixed labels. The fixture Host
   supplies hermetic metadata.
2. Append the short build id within the fourth `*IDN?` field; preserve the existing reply
   when the file is missing, unreadable or empty. Do not add file reads to query paths.
3. Test the exact four-field reply and fallback, cached reads and MCP forwarding. Check
   the identity and MCP consumers in `tools/device-test.py` without accessing hardware.
4. Run focused debug/release tests and `cargo xtask check --fast`; prepare the commit
   message and leave the changes uncommitted for coordinator-arranged standard review.

## Status

- 2026-11-15: author handoff ready on the uncommitted `t0053-idn` candidate based on
  `18ac5c72ba5e268905ccd583af4cb00f91a0ec80`. Final command
  `CARGO_NET_OFFLINE=true cargo xtask check --fast` exited 0:
  `partial gate passed (not done): 8 steps in 21 s`. Complete output:
  `target/0053-fast-gate-final.log` and `target/check-logs/78425-1794754932449807000/`.
  Full gate, coverage, performance, ARM and image checks were not selected, as requested.
  No ratchets changed. `target/COMMIT_MSG.txt` is prepared; no commit or push.
  Coordinator next action: standard cross-vendor review, then the full integration-batch
  gate. Task remains active until review and integration.
- 2026-11-15: final focused checks exited 0 in both debug and release:
  `cargo test --locked --offline -p oos-ui --bin oos-ui idn_` (4 passed) and
  `cargo test --locked --offline -p oos-ui --bin oos-ui mcp_session` (1 passed),
  each repeated with `--release`. Complete logs: `target/0053-idn-debug-final.log`,
  `target/0053-idn-release-final.log`, `target/0053-mcp-session-debug.log` and
  `target/0053-mcp-session-release.log`. Python compatibility fixture command
  `python3 -B target/0053-device-consumers.py` also exited 0 for old and new replies;
  output is in `target/0053-device-consumers.log`. Hardware and live socket transports
  were not verified; these checks use in-process fixture hosts and MCP calls.
- 2026-11-15: implemented the startup Host cache and four-field SCPI firmware identity;
  added fixture-host reply, fallback, delimiter and MCP forwarding tests. Cached-query
  testing replaces the text reader with a panic after construction. The build/MCP tests
  failed before the fix (1 passed, 2 failed, exit 101); the initial focused debug/release
  runs pass (4 tests each). Final candidate evidence is recorded above. The token cache
  rejects response delimiters and tokens over 128 bytes, with fallback tests.
- 2026-11-15: initial fast gate exited 1: `gate FAILED in 20 s: size`, reporting
  `ui: impls ui Ui grew: 33 -> 34 (ratchet)`. Moved the accessor into the existing
  `ui/src/ui/sysinfo.rs` impl, added that unheld path to the claim, and kept all ratchets
  unchanged. Complete initial output: `target/0053-fast-gate.log`; final rerun passes
  as recorded above.
- 2026-11-15: inspected `tools/device-test.py` identity/MCP consumers and the UI MCP
  forwarding path. Executed the actual Python consumer functions with synthetic SCPI
  and MCP fixtures for both old and build-bearing replies: both pass, exit 0. No device
  or network access; hardware behavior remains unverified.
- 2026-11-15: claimed by Codex for the owner-assigned `t0053-idn` worktree, base
  `18ac5c72ba5e268905ccd583af4cb00f91a0ec80`. Review tier: standard. The assigned
  scope authorizes this claim; no commit, push or hardware use. Avoid the `ui.rs` hot
  file held by task 0035 by using the Host and system-information seams.
- 2026-11-15: seeded by the coordinator.
