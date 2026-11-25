---
id: 307
title: Boot card mode and trial evidence
kind: feature
status: active
release: none
owner: codex
depends: []
---

## Goal

Expose boot card mode in System and base SDR104 fallback on an observed trial start.

## Plan

Reserved paths: `hal/src/update/`, `hal/tests/rootfs_safe_start.rs`, `fpgad/src/bin/oos_update/`, `fpgad/src/bin/oos-update.rs`,
`hal/src/bootcard/mod.rs`, `xtask/data/ui/size.txt`, `rootfs/initramfs/etc/rcS`, `ui/src/ui.rs`, `ui/src/ui/menu.rs`,
`ui/src/ui/menu/system.rs`, `ui/src/ui/menu/system/`, `ui/src/ui/active_choice/system.rs`, `ui/src/ui/menu/picker.rs`,
`ui/src/ui/menu/navigation.rs`,
`ui/src/ui/host.rs`, `ui/src/ui/host/tests/sd_card.rs`,
`ui/src/ui/update.rs`, `ui/src/ui/update/`, `ui/src/ui/power.rs`,
`ui/src/ui/power/`, `ui/src/ui/time.rs`, `ui/src/sysconf.rs`, `ui/tests/golden/`,
`docs/update.md`, `docs/boot.md`, `docs/known-issues.md`, `docs/process/lessons.md`,
`docs/spec/menus.md`, `wiki/Troubleshooting.md`, `CHANGELOG.md`.

- Use the negotiated timing and running tree with the update handoff and trial markers.
- Group the card controls with Boot mode and Update in System. Confirm the restage, report its outcome and
  link to the controls from the slow-card notice.
- Restore the Storage panel and its references from main.
- Check power cuts with the card fake, exercise menu paths and verify host gates.

## Status

- 2026-11-25: Formatting and the 12-step fast gate exited 0. The four selected checks exited 0 with
  3,838 release tests passed, Clippy passed, 77 matching register plans and all performance guards passed.
  Acceptance on a DHO924S and independent review remain open.
- 2026-11-25: Restored the Storage references from main. Repeated host captures updated only the System
  layout, System behind About, System after a Quick key selection, and the slow-card notice.
- 2026-11-25: Codex took the local C7 handoff on `feat/t0307-boot-card-mode` at `425470a3`.
