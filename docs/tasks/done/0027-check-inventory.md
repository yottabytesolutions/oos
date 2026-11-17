---
id: 27
title: "Define and enforce the full check inventory"
kind: chore
status: done
release: 0.2.0
owner: codex-c
depends: []
---

# Define and enforce the full check inventory

## Goal

Make full gate completion mean the same thing locally, in CI and in review.

## Plan

Reserved paths: none. Reserve paths when claimed.

Planning only on `plans-xtask`; implementation claims are still required. Serial order:
**0043 (G1) -> 0020 -> 0025 -> 0027 -> M2**. Start `t0027-check-inventory` from the integrated 0025 result.
All three tasks share `xtask/` and `docs/development.md`; finish each review/integration before the next claim.

### Inventory and completion contract

1. Add `xtask/src/inventory.rs` with stable check IDs, commands, prerequisites, execution order and selection
   (fast, full, conditional image). The registry drives execution and the printed inventory, rather than a second
   documentation-only list. Add `check --list` to show required IDs and conditions without running checks; it is
   not a passing gate. Keep `--fast` and `--no-coverage` explicitly partial. Add `check --image` for the full host
   gate plus the local image suite. Do not add an option that silently omits a mandatory check and says done.
2. Preserve G1's seven fast steps: fmt, size, architecture, mutable statics, unsafe, disallowed-method exceptions
   and known failures. Retain 0020's agent-rule step and 0025's static pin checks. The full inventory includes
   doctor, host/target clippy, Rust debug/release tests, perf guards and coverage, with G1's per-crate ratchets,
   move-aware comparison and 0.5-point margin unchanged. Reuse the current workspace-registry completeness check;
   task 0023 owns further protected-base/gate-weakening policy, and 0031 owns broad JSON/timing/timeout redesign.
3. Extend the default full `cargo xtask check` with the host checks below. Doctor requests 0025's GPU and
   SSH/firewall simulation profiles before launching them. Missing required tools, fixtures or outputs fail with
   setup instructions; unavailable Docker is never a passing skip. Fast mode requires no Docker and stays partial.

| Full check ID | Concrete command or behavior |
|---|---|
| `script-syntax` | Discover tracked Python and shell scripts recursively from Git's NUL-separated list, including extensionless shebang scripts, hooks and rootfs entry scripts. Compile Python without executing it, with bytecode under the selected target. Use the declared shell's `-n`; explicitly map sourced shell fragments. Include `boot/`, `boot/gel/`, `gpu/`, `kernel/`, `rootfs/`, `tools/`, `ui/tools/` and `.githooks/`. Fail on an unclassified executable script. |
| `python-tests` | `python3 -I -m unittest discover -s tools/tests -p 'test_*.py'` and the separate discovery in `tools/privacy`. Build a receipted host UI first with `python3 tools/output.py build -p oos-ui` for the socket/harness cases. Require nonzero test selections and report counts and skips. |
| `arm64-release-link` | `python3 tools/output.py build --release --target aarch64-unknown-linux-musl -p oos-ui -p oos-fpgad --bins`. Require all shipped Rust executables: `oos-ui`, `oos-fpgad`, `oos-update`. Verify receipts and AArch64 ELF/static linkage; this is code generation and linking, additional to G1's target clippy. |
| `gpu-host` | Run `sh gpu/test-host.sh` after the ARM link, using 0025's pinned arm64 llvmpipe image. Require every script in `ui/tests/*.txt` to produce matching CPU/GPU picture sets and pass the existing tolerance and all three monitor cost-per-pixel checks. Keep image differences for review. |
| `ssh-sim` / `firewall-sim` | Run `sh tools/ssh-sim.sh` and `sh tools/fw-sim.sh` for every full gate. Prepare the pinned initramfs builder (for its busybox) and nft/Dropbear/SFTP artifacts using their builders first; neither simulation needs a complete kernel/image build merely to get busybox. Keep all networking in disposable container namespaces. |

4. Make discovery deterministic and compare expected artifacts/test selections with actual outputs. Fix empty
   GPU comparisons and simulation scripts that only print a failed assertion but return success. In particular,
   GEL's printed checks must become assertions before its result can count. Preserve task 0026's unique run
   directories, socket readiness/PID checks, receipt verification and selected `CARGO_TARGET_DIR`; never consume
   an older default-target binary as a substitute. Use synthetic fixtures and no private calibration directory.
5. `check.rs` dispatches the inventory through small `xtask/src/steps/host.rs`, `scripts.rs` and `image.rs`
   modules. Adapt `run.rs`/`report.rs` only as needed to preserve command exit status, retain complete per-step
   logs under the selected target and show failed/missing/not-selected entries. No shell pipeline may turn a
   failed check into success. Fixture subprocesses test this independently of printed PASS text.

### Image validation and CI

- Add an always-created `image-required` classifier in `.github/workflows/ci.yml`. Compare the candidate with
  the authoritative PR base or push base, inspecting both paths of renames/deletions. Trigger for `boot/**`,
  `kernel/**`, `rootfs/**`, `gpu/**`, root/workspace Cargo manifests and lockfile, `rust-toolchain.toml`,
  `.cargo/**`, build/output/environment scripts, their tool/container locks, image/check runners and relevant
  workflow files. Explicitly include `ui/build.sh`, `tools/hw/**`, `tools/image-manifest.py`, `tools/output.*`,
  `tools/build-*`, `tools/bootstrap.sh`, `xtask/**` and all member `Cargo.toml` files. Missing base or an
  unclassified build input fails closed. Documentation-only changes do not need a new assembled image.
- Refactor `.github/workflows/image.yml` to be reusable by PR CI while retaining manual/tag entry points. A
  named `image-validation` job depends on the classifier and calls the same image-suite implementation exposed
  locally by `cargo xtask check --image`. Local ordinary checks print whether image evidence is required; if it
  is, a successful host run says image validation remains required for done. Never label that aggregate complete.
- The suite verifies/downloads the checksum-pinned kernel source, builds `kernel/build-base.sh`, assembles with
  `boot/build-dev-image.sh`, signs explicitly with `boot/sign-dev-image.sh --community`, packages with
  `python3 -I boot/build-gel.py`, and runs `tools/unit-dtb-check.sh`, `tools/gel-sim.sh`, `tools/usb-sim.sh` and
  `tools/wlan-sim.sh`. Use the same locked environments as 0025 and fresh worktree-local caches. Validate the
  boot image, DTB, initramfs, update and GEL contents, checksums and receipts, including `oos-hw` and the GPU
  bundle in the shipped artifact inventory. No install, device access or private signing-key discovery occurs.
- Keep the existing Linux arm64/x86-64 image matrix and strict manifest comparison, including its explicit
  exemption for host-dependent OOS executable bytes. Report that exemption rather than claiming byte-identical
  complete images; task 0033 owns stronger reproducibility. The local suite verifies one host; both CI matrix
  builds and the comparison are required image evidence. Retain manifests, tool identities and failure logs.
- Add an always-running `validation` aggregation job with explicit `needs` and `always()` handling. It requires
  the complete host gate, Privacy validation, and successful image matrix/comparison when triggered. Make
  `privacy.yml` reusable with `workflow_call` and invoke it as a job in this workflow so its result is available
  through `needs`; separate workflows cannot be direct job dependencies. Preserve its trusted-base scanner,
  incoming-range policy and restricted permissions. It accepts
  image non-applicability only from a successful classifier, never a cancelled/failed job or a path-filtered
  missing workflow. Use a named unambiguous result for required PR validation; the owner/coordinator configures
  branch protection, whose current settings have not been verified. Test the job's dependency/result logic.
- Update `docs/development.md` with the ID/command/prerequisite/condition table and aggregate done rule, and align
  `CONTRIBUTING.md`, `docs/fork/sync-checklist.md`, migration/extending gate references and
  `docs/process/review.md`. The old separate CI syntax step becomes part of xtask, not a second divergent gate.
  Review evidence includes the exact candidate, all applicable IDs/results and logs. Host success alone cannot
  close a change requiring image CI; separate revisions' passes cannot be combined as evidence.

Proposed implementation reservation: `xtask/src/inventory.rs`, `xtask/src/{main,check,report,run,target}.rs`,
`xtask/src/steps/{mod,host,scripts,image}.rs`, `.github/workflows/{ci,image,privacy}.yml`, `gpu/{test-host.sh,compare.py}`,
the six simulation/DTB scripts above, prerequisite preparation in `tools/build-env.sh`, new
`tools/tests/test_check_inventory.py`, and the documentation named above. Reserve further caller/test files
explicitly if the inventory exposes a defect; do not silently expand into application changes or weaken a check.

### Failing-before tests and acceptance

- On the post-0025 parent, reproduce that an invalid nested Python/rootfs shell script, broken GPU comparison
  and failing simulation are absent from xtask's executed checks. Add fixture regressions whose fake commands
  emit PASS then exit nonzero, fail to start, produce zero tests or omit a required output; the new full gate
  must fail. Positive fixtures verify command order, flags, expected check IDs and complete failure propagation.
- A tiny isolated cross-target fixture with an unresolved external symbol must pass `cargo check` and fail a
  release link, proving the former target check cannot establish linkability. Verify real ARM release artifacts
  for all three Rust binaries, including the secondary `oos-update` bin, and test missing/stale receipts and a
  nondefault output directory. No generated gate allowance or changed lint limit is part of this task.
- Test fast/default/no-coverage/list/image selection, doctor profile requirements, syntax paths with spaces,
  extensionless scripts, shell interpreter selection and empty discoveries. Test GPU missing/extra images and
  simulation assertion failures. Verify required tests use synthetic inputs and a missing prerequisite fails.
- Fixture Git histories exercise image path additions, deletions, renames, lock/config/workflow changes,
  documentation-only changes and unavailable bases. Test aggregation for success, failure, cancellation,
  legitimate non-applicability and incorrectly skipped image jobs. A boot/rootfs PR must request image validation
  where today's ordinary Rust PR would not. Include a small broken image-content fixture that assembly validation
  rejects even when the file is nonempty.
- Record failing-parent and passing-candidate commands, debug/release xtask/tool tests, complete final
  `cargo xtask check` output and exit code, and all applicable image matrix evidence. Obtain independent Claude
  review of inventory omissions, trigger coverage and CI result aggregation under the review protocol. The
  coordinator reruns the full gate on the integrated candidate. No hardware is needed or verified.

Estimate: 3-4 engineering days plus cold image/emulation runs and review availability. Risks: full validation
now requires Docker and usable container networking; first builds are expensive, especially x86-64 emulation;
current simulations may reveal failures hidden by printed-only checks; platform-specific scripts need correct
interpreters. Keep fast feedback partial, measure cost for task 0031, and resolve failures without silent skips.
Re-read G1's integrated result and retain subsequent `overhaul` privacy/isolation changes before implementation.

Audit items: 14, 15.

Input: [multi-agent audit](../notes/audit-multiagent-20261114.md).

## Status

- 2026-11-14: seeded from task 0001. Not yet claimed.
- 2026-11-15: claimed for codex-c on branch t0027-check-inventory (on top of tasks 0025, 0020 and G1) by the coordinator.
- 2026-11-15: review round 1 fast gate exits 0, printing
- 2026-11-15: review round 1 fixes implemented. Default `doctor-base` invokes
- 2026-11-15: review regressions fail before the fixes: focused xtask suite exits
- 2026-11-15: review round 1 on committed candidate `57096f6`, branch
- 2026-11-15: follow-ups from tooling review, intentionally not implemented in this round:
- 2026-11-15: tier-adjusted candidate checks pass: `cargo xtask check --fast` exits 0,
- 2026-11-15: applied the coordinator's tier scope decision above. Earlier default-gate/CI
- 2026-11-15: final full command `cargo xtask check` on the uncommitted candidate
- 2026-11-15: final source identity is retained in `target/0027-final-source.txt`; the task
- 2026-11-15: implemented Plan 1-5: executable registry/list/selection, preserved G1/0020/0025
- 2026-11-15: failing-parent evidence: `cargo test --locked -p xtask host_failures_are_part_of_the_gate`
- 2026-11-15: candidate focused checks: 95 xtask tests pass in debug and release; 22 new Python
- 2026-11-15: full-tree privacy audit exits 1 with 151 pre-existing findings; this task's
- 2026-11-15: independent Claude review attempted read-only through the installed CLI; it
- 2026-11-15: reserved `xtask/src/coverage.rs` solely to route its environment-specific
- 2026-11-15: reserved `fpgad/tests/explicit_signing.rs` after the full gate exposed its
- 2026-11-15: reserved `tools/privacy/test_round2.py`: full Privacy discovery exposed a stale
- 2026-11-15: reserved `xtask/src/testrepo.rs` after debug tests exposed inherited host
- 2026-11-15: implementation authorized by owner in `t0027-check-inventory` on parent `18123ce`
- 2026-11-15: done. Merged into overhaul (batches integ-5 to integ-7) after cross-vendor review and a passing full gate.
