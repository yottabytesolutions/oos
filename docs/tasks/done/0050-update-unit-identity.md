---
id: 50
title: "Refuse updates built for another unit"
kind: fix
status: done
release: 0.2.0
owner: codex-d
depends: []
---

# Refuse updates built for another unit

> Historical unit-image references below are superseded by ADR 0015; use generic images and device state.

## Goal

An update package built without the unit's `OOS_UNIT` must not reach a kexec start on that unit by accident.

## Plan

Reserved paths: hal/src/update/, fpgad/src/bin/oos-update.rs, fpgad/src/bin/oos_update/, boot/unit-hook.sh,
boot/unit-hook-image.sh, boot/build-dev-image.sh (unit lines only), boot/sign-dev-image.sh, docs/update.md,
docs/fork/unit-dho804.md, their tests, ui/src/http.rs, ui/src/ui/update.rs, ui/src/ui/update/upload.rs and
tools/tests/test_update_web.js.

Coordinator extension (2026-11-15): the three web paths above are reserved for this task. Further strictly necessary,
unheld deliverable paths may be added and listed. Keep xtask/, rootfs/initramfs/, ui/src/main.rs, ui/src/store*,
ui/tests/golden* and ui/tests/soak.rs untouched. Apply the saved web patch, replace its checkbox with a separate
confirmation naming the package and running units, preserve CLI override logging, then run focused debug/release
tests and the fast gate. Keep all changes uncommitted; prepare target/COMMIT_MSG.txt.

Evidence (task 0049, 2026-11-15): an overhaul image built without `OOS_UNIT=dho804` carried the plain DTB
(no sd-uhs-sdr104/sdr50, no vccio_sd settling time, no local-mac-address). The dry run only said "device tree: another
one" (hal/src/update/mod.rs:444). The kexec start hung before ramoops registered; the hardware watchdog reset the SoC
about 20 s later and the unit came back on the previous build by itself.

Fix: record the unit name (or "none") in the signed package manifest at build time; the updater compares it with the
running image's unit and refuses a mismatch, including unit -> none, unless an explicit override flag is given (CLI
and web /update both). Older packages without the field: treat as "none". Keep accepting a different device tree for
the same unit (kernel/DTB updates). Document `OOS_UNIT` in docs/update.md (build, sign, install). Failing-before tests
for: mismatch refused, none on a unit refused, same unit with another tree accepted, override accepted and logged.

Review round 1 plan (2026-11-15): correct first-rollout and identity recovery instructions; inject the running
unit into install options, resolved by CLI/web callers; return HTTP 409 with explicit refused JSON for a mismatch;
verify the signing unit against the image initramfs. Extend existing regression tests, run focused debug/release
suites and the fast gate, and update target/COMMIT_MSG.txt. Same reservations and held paths apply.

## Status

- 2026-11-15: final round 1 verification: HAL update suite 78 passed, 2 existing private-image tests ignored in both
  debug and release; CLI/signing suite 23 passed in both; web update suite 8 passed in both debug and release.
  Six browser tests and one unit-staging test passed. Logs: target/t0050-r1-{hal,signing,web}-{debug,release}.log,
  target/t0050-r1-browser.log, target/t0050-r1-staging.log. Host and ARM64 all-target clippy commands exited 0;
  changed-file diagnostic counts across both targets fit the existing ratchets (target/t0050-r1-clippy-*).
  Commit-message privacy and working-tree additions privacy both report 0 findings; diff whitespace passes.
  Updated target/COMMIT_MSG.txt. No hardware access, commits or pushes.

- 2026-11-15: round 1 implementation addresses P2-1 (unguarded legacy rollout, manual unit check and recovery),
  P2-2 (caller-supplied InstallOpt.running_unit; CLI resolves once, card files use --unit or none; web caches its
  resolved identity and tests inject a fake), P3-3 (HTTP 409 and refused:true JSON, updated browser handling), and
  P3-4 (pack reads the initramfs identity and rejects conflicting --unit). Same reservations and held paths kept.
- 2026-11-15: failing-before evidence: the new plain-image signing test accepted --unit dho804 before
  the fix (exit 101, target/t0050-r1-before-pack.log). Restoring the original host-reading check with the new
  explicit-unit regressions caused both to fail (exit 101, target/t0050-r1-before-identity.log); restored the fixed
  implementation immediately afterward. The current browser regressions against HEAD's script fail two tests
  (exit 1, target/t0050-r1-before-browser.log). Fixed implementations pass focused debug suites.
- 2026-11-15: initial cargo xtask check --fast exited 1: "gate FAILED in 17 s: size limits". Ratchets flagged
  hal/src/update/mod.rs 726 -> 729, ui/src/http.rs 1148 -> 1149, Http::poll 105 -> 106 and finish_kexec 69 -> 70.
  Moved the option default next to the unit helpers, simplified HTTP status selection and inlined kexec options.
  No ratchet or held file changed. Rerun exited 0: "partial gate passed (not done): 5 steps in 12 s"
  (target/t0050-r1-fast.log). Full gate and renewed independent review remain open.

- 2026-11-15: round 1 fixes started on committed c99ee51, clean worktree. Claude review disposition: fix first,
  no P1. Changes stay uncommitted; no hardware access. Final independent review and full gate remain required.

- 2026-11-15: resumed verification passed, exit 0 for each command: focused HAL unit regressions (9), CLI/signing
  integration tests (20) and web update tests (7), each in debug and release. Commands: cargo test --offline --locked
  [--release] -p oos-hal --lib update::tests::unit; -p oos-fpgad --test explicit_signing; -p oos-ui --bin oos-ui
  ui::update::. Logs: target/t0050-resume-{hal,signing,web}-{debug,release}.log. The first debug web run reused the
  isolated draft's cached binary; cargo clean -p oos-ui --profile dev followed by a fresh run verified all 7 current
  tests. Browser interaction tests: node --test tools/tests/test_update_web.js, 4 passed; unit staging test:
  python3 tools/tests/test_update_unit.py, 1 passed. Logs: target/t0050-resume-browser.log and
  target/t0050-resume-staging.log.
- 2026-11-15: cargo xtask check --fast exited 0: "partial gate passed (not done): 5 steps in 12 s". Formatting, size
  limits, architecture, mutable statics and unsafe passed (target/t0050-resume-fast.log). Focused host UI clippy
  exited 0 with no update-module findings (target/t0050-resume-clippy.{json,log}). Commit-message privacy and diff
  whitespace checks passed. Extended target/COMMIT_MSG.txt and replaced the stale handoff with current evidence.
  Held paths untouched; no hardware access, commits or pushes. Full gate and independent review remain open;
  the earlier full gate failures below are not superseded by this partial pass.

- 2026-11-15: applied target/t0050-web.patch and replaced the draft checkbox with a structured mismatch response
  and separate browser confirmation naming both units. Cancel retains no package; confirm resends the captured file
  with any-unit=1. Additional unheld path: tools/tests/test_update_web.js, needed to exercise confirmation, cancellation,
  per-upload consent and error handling against the actual embedded JavaScript without network or hardware access.

- 2026-11-15: resumed on coordinator authorization; web reservation extended to ui/src/http.rs, ui/src/ui/update.rs
  and new helper ui/src/ui/update/upload.rs. Saved patch apply-check passed. Confirmation and verification pending.

- 2026-11-15: final targeted release runs passed: 9 core regressions (`target/t0050-core-release.log`) and 20 CLI/signing integration tests (`target/t0050-signing-release.log`), matching debug results. Final web draft: 6 tests passed in the isolated source copy. Draft patch SHA-256: `606b70d031fb0c66b8310b9ee71071a20cc79082274cd4bca54931f8b06610ed`; apply-check passes. Review notes and commit-message draft are in `target/t0050-HANDOFF.txt` and `target/COMMIT_MSG.txt`. No source web edits, hardware access, commits or pushes.

- 2026-11-15: full `cargo xtask check` exited 1: `gate FAILED in 840 s: clippy, tests (debug), tests (release), coverage`. Complete output: `target/t0050-gate.log`. Formatting, size, architecture, statics, unsafe and perf guards passed. The initial new helper lint and handoff expectation were fixed and pass targeted checks. Existing WLAN/VXI/web socket tests fail with `Operation not permitted`; the xtask temporary Git-commit fixture fails too. Coverage stops on the WLAN socket failure, so no coverage pass is claimed. Task is not done; final full gate, web claim extension and independent review remain open.

- 2026-11-15: final fast gate passed all 5 steps, exit 0, 18 s (`target/t0050-fast-final.log`); host clippy found no new-module findings (`target/t0050-clippy-final.*`). Web draft compiled in an isolated source copy under the same output root: 6 update tests passed (`target/t0050-web-draft-tests.log`). Draft remains unapplied pending the web reservation extension. Kexec override handoff regression passed separately.

- 2026-11-15: core implementation is uncommitted and host-only. `cargo test --offline --locked -p oos-hal update::tests::unit --lib`: 9 passed; signing/CLI integration suite: 20 passed; unit staging Python test: 1 passed. Shell syntax, diff whitespace and commit-message privacy passed; added-line privacy scan found 0 issues (whole-tree audit still reports 153 existing findings).
- 2026-11-15: fast gate passed all 5 steps, exit 0 (`target/t0050-fast.log`). Full gate is running (`target/t0050-gate.log`); initial new helper lint and command-line expectation failures were corrected. Existing socket tests fail with `Operation not permitted`, and an existing xtask temporary-commit fixture also fails in this sandbox. No gate rules or ratchets changed.
- 2026-11-15: required web override needs `ui/src/http.rs` and `ui/src/ui/update.rs` plus a helper module, outside the original reservation. Claim extension requested; draft is confined to `target/web-draft/` and `target/t0050-web.patch` pending owner/coordinator approval. Cross-vendor review, final gate and hardware verification remain open. New state is one unit profile (maximum 64 ASCII bytes, immutable image lifetime) and one override boolean per install; existing report ownership and queue limits are unchanged.

- 2026-11-15: codex-d (Codex), branch `t0050-update-unit`, base `5df2722134ec95d6bfb9d0866dbec795014be07a`. Host-only implementation; no commit or hardware access. Plan: signed `unit` extension, build-time identity file plus receipted sidecar, shared pre-install check and explicit logged `--any-unit`. Web claim extension requested.
- 2026-11-15: failing-before evidence: `cargo test --offline --locked -p oos-hal update::tests::unit_tests -- --nocapture` exited 101; both foreign-unit direct install and kexec staging were accepted. Full output in `target/t0050-before.log`.

- 2026-11-15: seeded and claimed for codex-d by the coordinator from the task 0049 finding.
- 2026-11-15: done. Merged into overhaul.
