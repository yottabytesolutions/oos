---
id: 25
title: "Pin build tools and add a doctor command"
kind: chore
status: done
release: 0.2.0
owner: codex-c
depends: []
---

# Pin build tools and add a doctor command

## Goal

Give contributors and CI the same tested toolchain and prerequisites.

## Plan

Reserved paths: none. Reserve paths when claimed.

Planning only on `plans-xtask`; implementation claims are still required. Serial order:
**0043 (G1) -> 0020 -> 0025 -> 0027 -> M2**. Start `t0025-toolchain-doctor` from the integrated 0020 result;
release its overlapping paths only after review and integration. 0027 consumes the doctor profiles and pins.

### Pins and build environments

1. Add root `rust-toolchain.toml`: exact `channel = "1.99.1"`, `profile = "minimal"`, components `rustfmt`,
   `clippy`, `llvm-tools-preview`, and target `aarch64-unknown-linux-musl`. There is no toolchain file today.
   The planning host reports Rust/Cargo 1.99.1, Python 3.14.10 and cargo-llvm-cov 0.9.1. Use these as initial
   validation candidates, not evidence of a passing gate on other hosts. Pin cargo-llvm-cov to `=0.9.1` with a
   locked install. Validate the exact numbered Rust distribution on macOS arm64 and Linux x86-64 before adoption;
   do not substitute floating stable if it or a component is unavailable.
2. Add `.python-version` with `3.14.10` and `tools/build-tools.json` for the coverage/Python versions and named
   prerequisite profiles. Keep the Rust pin authoritative in `rust-toolchain.toml`; test consistency wherever CI
   needs to read it. CI explicitly installs the pinned Python patch release, since a version file alone changes
   no interpreter. Doctor checks the interpreter actually launched as `python3`. Required host scripts use the
   standard library today, including `gpu/compare.py`; keep optional analysis environments outside the gate.
   Container Python and Mesa's Mako/YAML/packaging/PLY dependencies belong to the container lock, not host pip.
3. Add `tools/container-images.json` and `tools/build-env.sh` as the common image resolver/builder. Lock real,
   verified OCI manifest digests and platforms for the existing Ubuntu 26.04 and Alpine 3.22 bases; never invent
   digests in the plan. Record each derived role's recipe hash and tested package-version manifest. Constrain
   package installation to those versions and reject drift instead of accepting whatever apt/apk serves. A local
   tag is only an alias: reuse requires the matching base, architecture, recipe and package identity. Prepare
   missing images explicitly during bootstrap/build, not during the read-only doctor inventory.
4. Apply that contract to `boot/build-dev-image.sh` (arm64 initramfs builder and Alpine arm64 probe),
   `gpu/build.sh` (arm64 Mesa/Panfrost builder) and `gpu/test-host.sh` (arm64 llvmpipe test image). The last currently
   builds a native-host container but executes an arm64 UI: specify `linux/arm64` for both build and run and verify
   emulation on x86-64. Preserve Mesa 26.0.8, source version 26.0.8-1ubuntu0.3 and its existing checksum.
5. Cover delegated build environments too: `kernel/build-base.sh` supplies the native/cross kernel compiler and
   `oos-hw` compiler; `rootfs/build-{wlan-tools,fsck,kexec,nft,dropbear,sftp-server}.sh` use Alpine. Resolve their
   bases and derived compiler/package identities through the same lock. Update simulation image references when
   their old local initramfs tag is replaced. Include the suggested binfmt installer image identity in setup
   instructions; doctor must never run a privileged registration command.
6. Land the tool and container identity contract here so 0027 can rely on it. Coordinate with task 0033, which
   retains archive/package snapshot retention, source timestamps and end-to-end payload reproducibility.
   Base-image digests alone do not freeze apt/apk packages or make an OOS image reproducible. If exact packages
   are unavailable, stop adoption with a prerequisite failure and choose a retained package source or a tested
   immutable build-image digest through review; do not fall back to mutable repositories or broaden versions.

### Doctor, setup and G1 integration

- Add `xtask/src/doctor/` split into pin parsing, probes and tests, and register `cargo xtask doctor` in
  `xtask/src/main.rs` with explicit dispatch alongside G1's `check` and `ratchet --move`. Return nonzero for any
  missing/mismatched required prerequisite, and usage exit 2 for invalid flags. Report every independent finding,
  expected/actual version and an exact setup command. Doctor installs nothing and accesses no scope or keys.
- Default doctor checks Rust/Cargo identity and active rustup override, pinned components, the musl target's std,
  rustfmt/clippy versions belonging to that toolchain, cargo-llvm-cov 0.9.1, usable LLVM coverage tools and Python
  3.14.10 with required stdlib imports. Resolve `rust-lld` inside the active sysroot's host `bin` directory;
  it need not be on PATH. Probe it and validate the existing static/Cortex-A72 target configuration. A real
  release link of shipped binaries belongs to 0027, so linker presence is not advertised as proof of linking.
- Also check Git and the protected base ref G1 needs, `core.hooksPath = .githooks`, writable worktree-local output
  selection through `tools/output.py target`, and lockfile presence. Report only repository-relative diagnostics
  or placeholders, not machine names, home paths, environment dumps or credentials. Hook setup is explicit once
  per clone, not an automatic configuration change by doctor.
- `doctor --gpu` additionally requires a running Docker daemon, matching locked GPU test image and successful
  execution of a disposable arm64 probe, with software EGL/GLES available. `--sim` requires the pinned initramfs
  image, the SSH/firewall simulation utilities and a disposable container with working NET_ADMIN/nftables/IPv6.
  `--image` includes those profiles plus kernel source/checksum, kernel/native-or-cross C tools, rootfs builder
  identities, DTB/cpio/compression utilities and `qemu-system-aarch64` for USB/WLAN simulations. Check tools inside
  their build container where that is where they run. Default doctor lists unselected profiles as not checked;
  selected missing prerequisites always fail. Probes use private scratch space and clean up their own resources.
- Add explicit `tools/bootstrap.sh` to install the pinned Rust components/target and coverage tool, fetch locked
  Cargo dependencies and prepare selected container roles. Document installing the exact Python interpreter and
  Docker/QEMU on supported hosts before invoking it; never auto-install Docker, register binfmt or use sudo.
  Finish setup with doctor. Rustup may fetch the toolchain before xtask starts, so document a manual rustup
  prerequisite command for bootstrap failures that doctor itself cannot diagnose.
- Add the base doctor step to the full `cargo xtask check` in G1's `check.rs`, before costly checks. Keep fast
  checks usable without coverage or containers; validate committed pin/lock syntax there without running those
  probes. 0027 will request the additional GPU/simulation profiles as their checks become mandatory. Preserve
  G1's per-crate store/moves, seven fast steps, coverage margin and newer `overhaul` output/privacy behavior.
- Replace floating stable and unversioned cargo-llvm-cov setup in `.github/workflows/ci.yml` and `image.yml` with
  the shared pins and doctor commands; align the Python setup in `privacy.yml`. Update `docs/development.md`,
  `gpu/README.md`, `kernel/README.md` and `tools/OUTPUTS.md` for setup, image/cache identities and failure remedies.

Proposed implementation reservation: the root pin files, `tools/build-tools.json`, `tools/container-images.json`,
`tools/build-env.sh`, `tools/bootstrap.sh`, `xtask/src/doctor/`, `xtask/src/{main,check,target,coverage}.rs`, the
three workflows above, build/simulation scripts and documentation explicitly named above, and relevant new
`tools/tests/test_build_tools.py` tests. Resolve any overlap with 0033 or other accepted claims before claiming.
Use existing Rust dependencies and Python stdlib; no lint, ratchet or architecture-rule relaxation.

### Failing-before tests and acceptance

- First show G1 rejects the new doctor command as unknown while a gate run has no tool-version check. Add fake
  process probes for wrong Rust/Python/coverage versions, missing components/target/LLVM tools, absent sysroot
  linker, mismatched hooks, unavailable base ref and invalid output directory. Test that all failures are visible,
  including commands that print a plausible version but exit nonzero. Correct toolchains pass without a PATH lld.
- Fixture repositories and fake Docker responses cover missing/offline daemon, incorrect digest/platform,
  changed build recipe/package set, missing emulation and missing image/QEMU prerequisites. Mutation tests prove
  that today's tag-only cache checks accept the wrong image and the new identity checks reject it. Assert doctor
  never installs, pulls, registers binfmt, signs or contacts hardware; only selected profiles execute probes.
- Test CLI dispatch and fast/full selection, pin agreement across scripts/CI, exact locked install arguments,
  and arm64 build/run selection on an x86-64 host. Reuse output-isolation tests, including custom target paths and
  Mesa-cache survival. A compiler/image identity change must invalidate dependent build-cache receipts; preserve
  caches used by active builds and rebuild under a new identity rather than deleting shared caches.
- Run xtask and tool tests in debug/release as applicable, doctor on both supported hosts and every selected
  profile on capable runners, then the complete post-0020 `cargo xtask check` on the final candidate. Record
  versions, complete logs/exit codes and parent/candidate revisions for independent Claude review. The coordinator
  reruns the integrated gate. No hardware is needed or verified; building an image gives no install permission.

Estimate: 2-3 engineering days plus cross-host/container build time and review availability.
Risks: the numbered toolchain or package versions may not be obtainable on every runner; exact Python patch setup
adds contributor work; arm64 emulation is slow; cached Mesa/rootfs outputs currently omit environment provenance.
Prove cache invalidation and pinned-host compatibility before calling the pins tested. Reconcile G1's final merge
with later output/privacy work rather than replacing newer files wholesale.

Audit item: 9, with container identity prerequisites for item 18/task 0033.

Input: [multi-agent audit](notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
- 2026-11-15: claimed for codex-c on branch t0025-toolchain (on top of task 0020 and G1) by the coordinator.
- 2026-11-15: Scope adjustment implemented without staging, commits, push, hardware access or held-path edits.
- 2026-11-15: Final fast gate: `partial gate passed (not done): 8 steps in 68 s`, exit 0
- 2026-11-15: Doctor reports `expected >=3.12; actual 3.14.10` and accepts that interpreter. It rejects only
- 2026-11-15: Coordinator narrowed 0025 to release 0.2.0 tool/bootstrap/doctor and base/recipe identities.
- 2026-11-15: Implementation handoff, still active and not merge-ready. Author: Codex (OpenAI). Parent
- 2026-11-15: Pins plan items 1-2 implemented: exact Rust 1.99.1/minimal/components/musl target, Python 3.14.10,
- 2026-11-15: Pins plan items 3-6 implemented as a fail-closed identity contract and script wiring. The container
- 2026-11-15: Doctor/setup/G1 plan implemented in separate doctor modules and Python stdlib helpers. Default
- 2026-11-15: Failing-before evidence: parent doctor exits 2 (target/t0025-before-doctor.log); CLI regression
- 2026-11-15: Passing local evidence, all exit 0: xtask debug/release 89 tests each
- 2026-11-15: Full gate attempt printed exactly: gate FAILED in 917 s: doctor, tests (debug), tests (release),
- 2026-11-15: Changed-file privacy and draft message scans each report zero findings
- 2026-11-15: Claimed implementation on the assigned stacked branch. Parent doctor command exits 2 (unknown
- 2026-11-15: done. Merged into overhaul (batches integ-5 to integ-7) after cross-vendor review and a passing full gate.
