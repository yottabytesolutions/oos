---
id: 50
title: "Refuse updates built for another unit"
kind: fix
status: active
release: 0.2.0
owner: codex-d
depends: []
---

# Refuse updates built for another unit

## Goal

An update package built without the unit's `OOS_UNIT` must not reach a kexec start on that unit by accident.

## Plan

Reserved paths: hal/src/update/, fpgad/src/bin/oos-update.rs, fpgad/src/bin/oos_update/, boot/unit-hook.sh,
boot/unit-hook-image.sh, boot/build-dev-image.sh (unit lines only), boot/sign-dev-image.sh, docs/update.md,
docs/fork/unit-dho804.md, their tests.

Evidence (task 0049, 2026-11-15): an overhaul image built without `OOS_UNIT=dho804` carried the plain DTB
(no sd-uhs-sdr104/sdr50, no vccio_sd settling time, no local-mac-address). The dry run only said "device tree: another
one" (hal/src/update/mod.rs:444). The kexec start hung before ramoops registered; the hardware watchdog reset the SoC
about 20 s later and the unit came back on the previous build by itself.

Fix: record the unit name (or "none") in the signed package manifest at build time; the updater compares it with the
running image's unit and refuses a mismatch, including unit -> none, unless an explicit override flag is given (CLI
and web /update both). Older packages without the field: treat as "none". Keep accepting a different device tree for
the same unit (kernel/DTB updates). Document `OOS_UNIT` in docs/update.md (build, sign, install). Failing-before tests
for: mismatch refused, none on a unit refused, same unit with another tree accepted, override accepted and logged.

## Status

- 2026-11-15: seeded and claimed for codex-d by the coordinator from the task 0049 finding.
