---
id: 54
title: "kexec start with a changed device tree hangs"
kind: fix
status: active
release: 0.2.0
owner: codex
depends: []
---

# kexec start with a changed device tree hangs

## Goal

Updates that bring another device tree start through kexec like same-tree updates.

## Plan

Reserved paths: `docs/tasks/0054-kexec-dt-change-hang.md`, `hal/src/update/kexec.rs`,
`hal/src/update/kexec/`, `fpgad/src/bin/oos-update.rs`, `boot/build-dev-image.sh`,
`tools/image_suite.py`. Branch: `t0054-kexec-dt`.
Review tier: unit. Step 2 is host implementation only, without commits, hardware access or publication.
Scratch checks, logs and proposed commit message belong in this worktree's ignored `target/`.

1. Preserve the host analysis below and add the owner's LCD evidence.
2. On the tree-aware legacy load only, keep bounded debug stdout/stderr, exact staged lengths and the final Rust
   merged DTB on persistent `/oos` storage. Flush files and directory before shutdown; overwrite the previous attempt.
3. Remove the ineffective watchdog set/readback/ping experiment and its tests. Keep durable preparation and immediate
   jump timestamps. Add `OOS_KEXEC_NO_CHECKS=1` as a build-time-only diagnostic switch, default off: only the legacy
   argv gains exactly `-i`. Record the switch in persistent evidence. Keep normal argv and the same-tree path intact.
4. Test both argv variants, build-time selection, same-tree isolation and bounded evidence in debug and release;
   run `cargo xtask check --fast`, without the full gate (sandbox sockets are blocked). Update the one-diagnostic-build
   unit procedure and replace the single proposed commit message. Full gate and cross-vendor unit review remain
   coordinator work. No commits or unit access.

Unit evidence (2026-11-15, private RE notes docs/logs/oos_pcie_gen1_20261115/ and oos_unit_round0_20261115.md): twice the kexec_load path (kexec-tools with --dtb, taken when the image's tree differs) hung after the old kernel printed "Bye!", before any new ramoops registration; the hardware watchdog reset the SoC about 20 s later and the previous build came back. The same-tree path (kexec_file_load) works. Capture UART2 (115200) during a kexec start on the unit (needs the owner to attach the serial adapter), compare the loader-change merge of the tree (hal/src/update/fdt.rs with_loader_changes) with the running tree, and check memory placement of the dtb/initrd in kexec_load. Until fixed, DT-changing updates install with --no-kexec and a reboot.

### Host analysis, 2026-11-15

**Result: the hang's cause is not confirmed.** The evidence does not locate it inside the new kernel:
`Bye!` precedes relocation and userspace purgatory too. Absence of a new ramoops record is consistent with any
failure before its registration, or with damage to its reserved memory. The roughly 20 s reset fits the watchdog. The later LCD video below confirms a SoC reset and cold boot;
it does not independently identify the watchdog as the reset source. No unit was accessed for this analysis.

The strongest comparison is Gen1 attempt 1: its kernel hash is unchanged, the raw trees differ only in
`/pcie@f8000000:max-link-speed` (2 to 1), and a direct install and restart of that image succeeded. Round 0 attempt 1
also changed UHS and unit properties; that is not needed to reproduce the failure. Changing the tree currently
changes several variables together: tree construction, allocator, load syscall and purgatory.

The path in this revision is:

- `hal/src/update/mod.rs:429`: compare whole RSCE sections, not normalized DTBs. Equal gives `Staged.tree=None`;
  different extracts `rk-kernel.dtb` with `rsce_file` and applies `with_loader_changes`.
- `fpgad/src/bin/oos-update.rs:418`: `None` calls `kexec::load` (syscall 294, kernel/initrd/cmdline only);
  `Some` calls `load_with_tree`. Both then stop the watcher/UI and quiesce network/display before the jump.
- `hal/src/update/kexec.rs:244`: write Image/initrd/merged DTB to temporary files, then run
  `/bin/kexec -c --mem-min=<floor> -l Image --initrd=... --dtb=... --command-line=...`.
  The floor is at least 1 GiB, increased from GIC/ITS table addresses in the current kernel log. Successful tool
  stdout/stderr is discarded and the files are removed, so the existing evidence contains no segment map or final
  tool-mutated DTB. The reported 33 changes describes the Rust merge, not the final handoff tree.
- `rootfs/build-kexec.sh`: kexec-tools 2.0.32, SHA-256
  `8f81422a5fd2362cf6cb001b511e535565ed0f32c2f4451fb5eb68fed6710a5d`, static arm64 build with
  `--without-zlib --without-lzma --without-xen`, `CFLAGS=-Os`. The local source archive matches that pin.

### Owner LCD evidence and step 2, 2026-11-15

The owner reviewed the LCD video of the **16:44 reproduction**: after `Bye!`, the screen went black, the Rigol
loader logo appeared, then OOS booted normally. The logo is drawn only by the stock loader. This establishes a
**SoC reset followed by a cold boot, not a slow kexec**. No fbcon output from the new kernel was visible. This is
owner-observed evidence, not an observation made by this host-only session; the reset source remains inferred.
The owner timed **26 s from the jump (screen black) to the Rigol logo**, consistent with the 21.5 s watchdog
period plus up to one 5 s kick interval. An immediate reset is ruled out. This supersedes the earlier approximate
20 s description; it does not distinguish slow verification from a checksum mismatch loop.

The step 2 diagnostic is confined to `kexec_load` (`--dtb`). The `kexec_file_load` implementation and its `start`
function remain unchanged; its caller takes no new watchdog or evidence action when `Staged.tree` is `None`.
The watchdog set/readback/ping experiment and its tests have been removed: the ioctl cannot extend the verified
21.5 s hardware maximum without kernel pings, and the normal kicker owns the single-open device (EBUSY).
It discriminates nothing here. The watchdog supervisor remains unchanged.

`OOS_KEXEC_NO_CHECKS=1 boot/build-dev-image.sh` builds an explicitly diagnostic image. The script accepts only 0 or 1,
defaults to 0, exports the value to Cargo, and warns when enabled. Rust reads it with `option_env!` at compile time;
there is no runtime environment or updater flag to enable it. The normal legacy argv remains byte-identical to
job-88's `-d -c --mem-min=... -l Image --initrd=... --dtb=... --command-line=...`. Only the diagnostic legacy argv adds
exactly `-i`, the pinned tool's `--no-checks` alias. `kexec_file_load` and its start function never read this switch.
The image validation suite clears it alongside `OOS_UNIT` so inherited diagnostic settings cannot enter that suite.
Cargo tracks the compile-time environment read, including switching back to an unset environment.

Verified in the cached pinned 2.0.32 source: `kexec/kexec.c:1066` documents `-i` / `--no-checks`, and
`update_purgatory` at `:688` writes the purgatory `skip_checks` symbol. This bypass is solely a cause-isolation
experiment. It is not a permanent fix or a production workaround.

Persistent storage reuses the `/oos` data partition used by crash logs ([boot](../boot.md#restart-after-a-panic-or-a-hang))
and update state ([update](../update.md)). One fixed directory, `/oos/update/kexec-last`, is overwritten by each
legacy load attempt. It is separate from `/oos/crash` so its larger DTB allowance cannot consume the crash archive's
1 MiB budget or be confused with a panic archive. Files and their directory are fsynced; the parent directory is
also fsynced when the attempt begins. The six fixed files have a combined content ceiling of **2 MiB + 320 KiB**:

| File | Evidence and bound |
|---|---|
| `merged.dtb` | Complete final Rust loader-delta merged tree passed to `--dtb`, at most 2 MiB; oversized trees warn and are not saved, never saved partially |
| `staged.log` | Exact kernel, staged initrd and merged DTB byte lengths, `--mem-min` and `purgatory_no_checks=true/false`, at most 64 KiB |
| `stdout`, `stderr` | Last 64 KiB of each `kexec -d -c -l` stream; pipes are fully drained concurrently, including on tool failure |
| `load.log` | Tool exit status, at most 64 KiB |
| `jump.log` | Boot identity and pre-remount monotonic/uptime timestamp, at most 64 KiB |

`merged.dtb` is the exact Rust merge output, **not** a dump of kexec-tools' later in-memory tree. The tool changes
bootargs, seeds and initrd addresses internally without rewriting its input file. Its final segment addresses and
sizes are in debug stderr; reconstruct its `/chosen` initrd range from those addresses before comparing trees.
A post-tool binary DTB dump would require separate tool instrumentation. Evidence failures warn and continue;
check current boot identity, tool status and diagnostics before interpreting possibly stale or absent files.

After sync and the read-only remount attempt, the legacy start records the immediate jump marker, then calls
`reboot(KEXEC)`. The marker goes to stderr, kmsg and `/dev/pmsg0`; no watchdog device is opened or pinged.
The configured ramoops pmsg area is 256 KiB; read `pmsg-ramoops-*` from pstore after recovery, before another reboot.
The immediate marker contains `CLOCK_MONOTONIC` plus `/proc/uptime`; `jump.log` preserves the earlier marker before
`/oos` becomes read only. Diagnostic-device write failures are reported to stderr.

### Next unit procedure for one diagnostic image (not executed)

1. Coordinator obtains cross-vendor unit-tier review and the required full host/image/extended-suite evidence.
   Build with `OOS_KEXEC_NO_CHECKS=1 boot/build-dev-image.sh`, then prepare the signed diagnostic package through the
   documented route. **First install and cold-boot this diagnostic updater** using signed-package `--no-kexec`
   under the owner's separate install/reboot go and lease. Merely carrying it in the target initrd does not change
   the old updater that performs the jump. This is one diagnostic installation followed by one changed-tree trial;
   no automatic retry or production use of `-i` is authorized.
2. Confirm the running diagnostic build identity, OOS re-arm and safe fallback with the owner. Take the task-specific
   lease/capabilities; keep renewing it. Save current boot_id, `/proc/uptime`, dmesg, `/proc/iomem`, running FDT and
   old/new RSCE privately. Do not use `kexec-check` as the experiment. Wait for OOSK re-arm and at least 60 s since
   the preceding reboot. Leave the watchdog and its kicker running normally.
3. With separate owner go, attempt one changed-tree signed package through ordinary
   `oos-update install /path/to/trial.oosupd --execute` (no `--no-kexec` or `--new-dt`). Keep the tested kernel,
   staged package/initrd and single-property Gen1 tree change comparable to the original reproduction. Capture the
   LCD continuously with an elapsed timer from jump/black to loader logo or first new-kernel output. A cold reset
   returns to the installed diagnostic build; a successful kexec trial can install and confirm the candidate normally.
4. After the first outcome, stop the round and collect `/oos/update/kexec-last/*`, pstore `pmsg-ramoops-*` and console
   records, `/tmp/oos-watchdog.log`, `/tmp/oos-update.log`, boot status, boot_id, uptime and dmesg privately before
   any further reboot or attempt. A wedged/unreachable unit requires the owner; no automatic recovery or retry.
   Release the lease. Do not power-cut before collecting retained RAM evidence if the unit is reachable.
5. Verify `staged.log` says `purgatory_no_checks=true`, and pair that attempt's old boot_id and jump marker with video
   and the recovered boot's logs. Monotonic values from different boots are not directly subtractable. The unit's
   uptime counter was observed to survive watchdog resets, so use recorded uptime and recovered early-boot timestamps
   to estimate the reset interval, subtracting elapsed cold-boot time; report it separately from the video timing.
   A missing pmsg record leaves only the pre-remount bound, not an exact jump time. Stale/absent evidence or a switch
   value of false invalidates this diagnostic comparison.
6. Interpret this single controlled comparison:
   - **Boots with no Rigol logo and no reset:** the checksum stage is the cause, either slow uncached hashing or a
     mismatch. The bypass does not distinguish those two. Next comes the permanent fix with integrity verification
     restored, not production use of `-i`.
   - **Still resets at about 26 s:** with checksum verification skipped, investigate beyond that verification stage:
     relocation, kernel entry and early boot. Check the saved placement, lengths and merged DTB first. The next
     diagnostic option is purgatory writing stage markers into reserved RAM, read after reset, with separate review
     and owner authorization. Marker allocation must respect existing reservations; it is not implemented here.
     UART progress is another option. Do not infer a new kernel reached entry without a marker proving it.

### Ranked hypotheses and discriminating checks

1. **Purgatory has not reached the kernel before the watchdog expires, or stops at checksum verification.**
   This is the largest unobserved difference from the working path. In the pinned tool, `image_arm64_load` loads
   purgatory, and `arm64_load_other_segments` makes `purgatory_start` the entry. `update_purgatory` hashes the kernel,
   initrd and DTB, including zero padding; purgatory hashes their destination memory again and loops forever on a
   mismatch. Linux's `machine_kexec.c` documents that file mode enters the kernel directly. Relocation disables the
   MMU/data caches before entry. Software hashing of the large trial initrd can therefore consume watchdog time;
   a corruption-induced checksum loop has the same external signature. The updater supplies no `--serial`, so
   purgatory's default UART sink is zero and its progress/error prints are silent.
   **Host check run:** inspect the pinned source, extract the embedded arm64 ELF from the cached kexec binary and
   disassemble it (`purgatory.ro`, `purgatory.dis`). The checksum and pre-kernel call sequence are present. The model
   kernel plus initrd exceeds 80 MiB; host elapsed time cannot establish uncached RK3399 hashing time or the actual
   failing checksum. The cached binary hash is `631abbdb747420669034a884de9cc0ba989eccc92a8d80986aa638e74876a935`,
   not a claim that the unit used that exact binary.
   **Unit confirmation:** under a separately authorized signed diagnostic build, capture UART2 and timestamps at
   purgatory entry, before/after verification, immediately before kernel entry and the last watchdog ping. Validate
   the RK3399 UART sink before relying on the tool's serial option. A checksum mismatch confirms corruption; entry
   without completion before reset supports the deadline hypothesis; a completed kernel-entry marker moves the
   investigation into early Linux. Do not disable integrity checks as a production workaround.

2. **A destination overlaps hidden GIC tables, reserved memory, or another segment; alternatively live DMA corrupts
   it after load.** The repository already records both an old-initrd reservation collision and a hidden GIC table
   collision (`docs/update.md`). Userspace allocation uses the running memory map, not all reservations a new tree
   could introduce. `--mem-min` avoids the previously observed low GIC tables, not arbitrary future reservations or
   ongoing bus masters. Display/network quiescing is common to both paths, so the old DRM warning alone is weak
   evidence of this cause.
   **Host check run:** execute the pinned `locate_hole`, `dtb_set_property` and `dtb_set_initrd` functions in a native
   harness, using recorded RAM banks, the cached Image header and an approximate trial initrd size. Both raw old and
   Gen1 trees, after a synthetic loader-initrd delta, produce kernel `0x40000000`, header memory size `0x1aa0000`,
   initrd `[0x41aa0000,0x45150541)`, DTB `0x45154000` (size `0x16b61`) and a model purgatory allocation
   `0x4516c000`. Assertions pass for the 1 GiB floor, segment separation and `/memreserve/` separation. Image flags
   `0xa` permit placement away from DRAM's base; its SHA-256 is
   `a07a05779c054262a690c49ec4349c882c54ee80f5e61b30ffce502fb1f08b98`, matching the evidence's prefix.
   These are model addresses, not recovered failed-install addresses; the purgatory size and initrd length are
   approximations. Raw OOS reserved regions (ramoops, trust OS and the initial zero-size logo) do not intersect the
   model. The model does not simulate DMA, relocation source pages or newly introduced reservations.
   **Unit confirmation:** retain full `kexec -d -c -l` output, Image header, exact staged lengths, `/proc/iomem`,
   GIC/ITS lines and the final tool-mutated DTB before jumping. Check half-open physical ranges against all memory
   banks, kernel `image_size`, both reservation forms and active GIC tables. A concrete overlap or checksum change
   identifies the defect. A load/unload check alone cannot detect corruption during shutdown.

3. **The loader-delta merge loses or retains incorrect `/chosen`, memory or reservation data.**
   `fdt.rs:294` intentionally filters `Set(/chosen, linux,initrd-*)`: the old addresses must not describe the new
   initrd. The pinned tool sets both new properties as 64-bit big-endian values after allocation, including when
   `/chosen` must be created. Thus absent initrd properties in the intermediate merged tree are expected.
   Node/property deltas replay by path; changed phandles and missing destinations fail before the jump.
   Reservation merging at `fdt.rs:333` only adds reservations, excluding additions that overlap the old ramdisk.
   **Host checks run:** existing FDT tests pass (4 plus the explicitly enabled private-fixture test). The saved
   2026-11-08 stock old/running/modified triple replays 34 changes and normalizes back to its running tree apart
   from the intentionally omitted initrd. This is an older stock fixture, not the failing OOS 33-change triple.
   The saved Gen1 raw RSCE pair parses/round-trips and normalizes to exactly one PCIe property difference. Its
   `/chosen` has bootargs/stdout-path, no initrd or seeds, and both reservation lists are empty; memory and
   `/reserved-memory` nodes match. Applying a synthetic initrd delta preserves that one-property difference.
   The tool-function harness then supplies the correct two new initrd addresses.
   **Confirmed edge cases, not a confirmed cause of this hang:** filtering only `Change::Set` leaves stale initrd
   properties already present in `new`, and copies them inside a newly added `/chosen`; reservations removed by
   firmware are restored from `new`. Three targeted assertions fail on the actual current FDT code in debug and
   optimized builds. All three pass on a scratch-only proposal. The first two initrd-property cases are normally
   overwritten by kexec-tools anyway. Neither raw Gen1 tree contains the stale data needed for these cases.
   **Proposed correction:** clear inherited initrd properties after all change variants, discard an identified
   old-initrd reservation inherited from `new` (exact or page-rounded size), and replay removed reservation tuples.
   `target/t0054/fdt-proposal.patch` illustrates this only; it is not applied to runtime source and is not ready
   for integration without review of reservation ownership and malformed-range handling.
   **Unit confirmation:** capture the exact old RSCE, new RSCE and running FDT together, replay with the actual Rust
   code, then compare the final tool-mutated `/chosen`, root memory banks, both reservation forms and loader display
   properties with file mode. A missing required loader delta, stale reserved initrd, or overwritten intended new
   property confirms this family. The exact failing running FDT was not present in the supplied evidence directories.

4. **Seed or other kexec metadata differs between loaders.** The kernel file loader refreshes `kaslr-seed` and
   `rng-seed`, removes crash-only properties, and adds `linux,booted-from-kexec` in `drivers/of/kexec.c`. The tool's
   `setup_2nd_dtb` refreshes an existing zeroed 8-byte KASLR seed; a nonzero seed it rejects would fail the load
   before `Bye!`. It does not provide all of the kernel file loader's metadata processing.
   **Host check run:** synthetic seed values survive the Rust merge unchanged, so it provides no separate seed
   hygiene. Neither saved raw Gen1 tree nor the older running fixture contains seeds. `kernel/config` disables
   `CONFIG_RANDOMIZE_BASE`, reducing KASLR's likelihood here. The cached purgatory also contains SIMD register saves
   in variadic `printf`; `-fno-tree-vectorize` does not prevent these. This is not proof of an FP trap: Linux 6.12
   enables EL1 FP/SIMD access during normal CPU setup, and the actual handoff control registers were not captured.
   **Unit confirmation:** compare sanitized metadata presence/lengths in the two final trees and obtain UART's first
   exception/progress point. If execution stops in purgatory, inspect exception level, CPACR/CPTR and relocated entry
   data. A diagnostic build using general registers only can isolate code generation after independent review.

5. **EFI-less arm64 requires the identical running DTB, or the PCIe/UHS change itself cannot warm-start.**
   **Host check run:** Linux 6.12's legacy kexec path accepts userspace segments and expects purgatory to supply the
   kernel/DTB addresses; there is no byte-equality requirement. Its documented DTB constraints are alignment, size,
   mapping attributes and correct platform description. Direct Gen1 boot succeeds with the same kernel, and the
   PCIe module is not loaded before ramoops, so a link-speed change is a weak explanation for this early loss.
   This does not prove arbitrary DT changes are safe on warm hardware.
   **Unit confirmation:** first compare the same kernel, initrd and command line through file mode and forced legacy
   mode with an equivalent running tree. If legacy mode fails too, tree inequality is not necessary. Then compare
   legacy mode with only the one Gen1 property changed. Capture UART for both, observing the normal lease, install,
   reboot and re-arm requirements. No such experiment was performed here.

### Diagnostic limitation and rejected shortcut

`oos-update kexec-check` always calls `kexec::load` at `fpgad/src/bin/oos-update.rs:465`. Its later message that the
tree differs does not exercise staging, `with_loader_changes`, `--dtb`, legacy placement or purgatory. Its successful
result cannot clear task 0054. A follow-up diagnostic correction should share install's validated staging/loader
selection while preserving load/unload-only behavior and leaving update state/card unchanged. Its regression test
must present a different-tree package to a fake loader port and require the tree-aware load, then unload; the current
unconditional call would fail that expectation. This diagnostic correction is proposed, not implemented here.

Do **not** switch to `kexec_file_load --dtb` as a fix: the pinned tool explicitly clears `arm64_opts.dtb` for file
mode, and the kernel's file-load syscall has no DTB argument. Linux copies `initial_boot_params` when constructing
its new FDT. Such a change would silently stop testing the new tree. Primary source references:
[kexec-tools 2.0.32 arm64](https://github.com/horms/kexec-tools/blob/v2.0.32/kexec/arch/arm64/kexec-arm64.c),
[Linux 6.12 FDT setup](https://github.com/torvalds/linux/blob/v6.12/drivers/of/kexec.c) and
[arm64 handoff](https://github.com/torvalds/linux/blob/v6.12/arch/arm64/kernel/machine_kexec.c).

### Reproduction and handoff artifacts

All generated checks and private tree data stay in ignored `target/t0054/`; no private data is added to fixtures.
`host.rs` imports the actual `hal/src/update/fdt.rs`; `host-proposal.rs` imports only the scratch proposal.
Build them with `rustc --edition 2021 --test` (and `-O` for the optimized runs), and run with
`OOS_ANALYSIS_OLD_RSCE` and `OOS_ANALYSIS_NEW_RSCE` pointing to the saved Gen2/Gen1 RSCE captures. Logs:
`host-before-{debug,release}.log` (6 passed, 3 expected failures, 1 ignored, exit 101),
`host-proposal-{debug,release}.log` (9 passed, 1 ignored, exit 0), `fdt-tests.log`, `stock-fixture-test.log`,
`tree-inventory.log`, `loader-check.log` and `purgatory.dis`. Run the original ignored fixture test with
`OOS_DT_OLD`, `OOS_DT_RUNNING`, `OOS_DT_NEW` and `OOS_DT_OUT` as its source comment describes.
`python3 target/t0054/check_loader.py` extracts and compiles the pinned C functions against host libfdt and reruns
both model layouts. Its simplified allocation model is deliberately not a complete emulation of the arm64 tool.

## Status

- 2026-11-15: Step 2 revision implemented on uncommitted job-88 work, parent
  `0d173bbf5f77adc8c09d8d100362a150fdfc6a6f`. Removed the watchdog ioctl/open/ping module and tests; retained bounded
  persistent debug output, exact staged lengths, complete Rust merged DTB and both jump timestamps. Added the
  default-off build-time `OOS_KEXEC_NO_CHECKS=1` diagnostic: legacy argv adds only `-i`, evidence records
  `purgatory_no_checks=true/false`, and normal image validation clears the switch. Updated the owner-observed
  26 s jump-to-logo timing and one-install diagnostic procedure with both outcomes and the reserved-RAM marker option.
  Focused `cargo test -p oos-hal [--release] update::kexec`: **11 passed** in each of six runs, all exit 0:
  switch unset, set to 1, then unset again, each in debug and release. For diagnostic tests only, set
  `OOS_KEXEC_EXPECT_NO_CHECKS=1` alongside the compile switch as the independent expected value; unset both for normal
  tests. The sequence verifies Cargo tracks both enabling and removing the build environment. Complete logs:
  `target/t0054/step2-revision-{default,diagnostic,restored}-{debug,release}.log`.
  `cargo check -p oos-fpgad --target aarch64-unknown-linux-musl` passed with switch 1 and 0, both exit 0:
  `target/t0054/step2-revision-arm-{1,0}.log`. Same-tree load/start/unload/loaded match HEAD byte for byte; None skips
  diagnostic preparation and uses the original start, with no caller build-switch read:
  `target/t0054/step2-revision-file-path-check.log`. Host build-script checks passed for unset/0/1 child-environment
  export and warning selection, invalid-value rejection, and image-suite environment clearing. The child shim stops
  before any image build; the suite check stops before kernel preparation. Log:
  `target/t0054/step2-revision-build-switch.log`. Shell/Python syntax and `git diff --check` passed.
  `cargo xtask check --fast` exit **0**, printed **`partial gate passed (not done): 8 steps in 34 s`**.
  Passed: fmt, size, architecture, mutable-statics, unsafe, method-exceptions, known-failures, agent-rules.
  All other inventory steps printed not-selected. Complete output: `target/t0054/step2-revision-fast-gate.log`;
  per-step logs: `target/check-logs/2840-1794760031384608000/`. No full gate run for this revision, as assigned;
  the earlier job-88 socket-blocked failure below is historical, not evidence for the revised source.
  `target/COMMIT_MSG.txt` replaced with one proposed message. Intentional remaining changes: this task,
  `hal/src/update/kexec.rs`, three modules in `hal/src/update/kexec/` (arguments, evidence, legacy),
  `fpgad/src/bin/oos-update.rs`, `boot/build-dev-image.sh` and `tools/image_suite.py`.
  Task remains active. No commits, unit access, image installation, merge, push or publication. Coordinator next:
  full gate where sockets work, cross-vendor unit-tier review and any separately owner-authorized diagnostic round.

- 2026-11-15: Owner revised step 2, building on uncommitted job-88 work: replace the ineffective watchdog experiment
  with a diagnostic-only purgatory checksum skip. Plan updated before code; reserved build-script paths above have
  no other active task claim. Preserve evidence and timestamps. Implement and verify locally only; coordinator owns
  full gate, cross-vendor review and any separately authorized diagnostic install.

- 2026-11-15: Historical job-88 evidence, superseded by the checksum-skip revision above: full `cargo xtask check`
  completed on the earlier watchdog source in this dirty worktree, exit **1**:
  `gate FAILED in 773 s: tests-debug, tests-release, coverage, python-tests`. Complete output:
  `target/t0054/step2-full-gate.log`; full per-step/subprocess output and `steps.log`:
  `target/check-logs/91292-1794758761300225000/`.
  Passed as printed: fmt, size, architecture, mutable-statics, unsafe, method-exceptions, known-failures,
  agent-rules, doctor-base, clippy, perf, script-syntax. Image-only inventory was not selected.
  Debug/release HAL: 345 passed, 1 failed, 8 ignored; failure is existing `wlan::tests::join_commands`
  (`hal/src/wlan.rs:436`, Unix socket bind: PermissionDenied / Operation not permitted). UI in both profiles:
  582 passed, 21 failed, 29 ignored; affected tests are HTTP/network/SCPI/SNTP/VXI and Bode generator sockets,
  with socket creation denied by this sandbox. Coverage's instrumented HAL run repeats the same WLAN failure
  (345 passed, 1 failed, 8 ignored); no coverage floor result is claimed. Python: 115 tests, 1 error in
  `test_host_sockets.HostSockets.test_two_runs_use_their_own_children_ports_and_storage`; host child exits before
  readiness because its TCP listener gets Operation not permitted. No test skip, ratchet edit or workaround applied.
  Coordinator next action: rerun the full gate where local sockets are permitted, obtain independent cross-vendor
  unit-tier review, then decide the next authorized experiment. The watchdog experiment is now superseded above.
  `git diff --check` passed. `target/COMMIT_MSG.txt` contains one proposed message. Intentional remaining changes:
  this task, `fpgad/src/bin/oos-update.rs`, `hal/src/update/kexec.rs` and three new modules in
  `hal/src/update/kexec/`. Task remains active; no commit, unit access, merge, push or publication.

- 2026-11-15: Step 2 implemented on the dirty `t0054-kexec-dt` worktree at `0d173bb`: legacy-only bounded persistent
  evidence, late timeout set/readback/ping and monotonic jump markers. Added the owner's 16:44 LCD evidence and unit
  procedure above. Source inspection confirms this ioctl cannot increase the physical 21.5 s maximum and normal
  single-open ownership likely yields EBUSY; the procedure explicitly preserves an inconclusive outcome.
  Focused `cargo test -p oos-hal [--release] update::kexec`: 10 passed in each profile, exits 0; complete logs
  `target/t0054/step2-tests-{debug,release}.log`. `cargo check -p oos-fpgad --target aarch64-unknown-linux-musl` exit 0,
  log `target/t0054/step2-scope-check.log`. File-load `load` and `start` function text matches HEAD byte for byte;
  caller performs no new IO for `tree=None` (`target/t0054/step2-file-path-check.log`).
  Final fast gate exit 0: `partial gate passed (not done): 8 steps in 24 s`; all eight selected steps passed,
  remaining inventory not selected. Complete output `target/t0054/step2-fast-gate.log`, per-step logs
  `target/check-logs/81420-1794758725915417000/`. Full-gate result is recorded above; no full pass or hardware result claimed.

- 2026-11-15: Owner assigned step 2 on base `0d173bb`: legacy-only watchdog/evidence instrumentation and host tests.
  Reserved paths expanded above; no other active task claims these source paths. No unit access, commits or publication.

- 2026-11-15: Host analysis delivered with five ranked hypothesis families, host results and unit confirmation
  checks above. No root cause or hardware fix claimed. Runtime code, unit behavior and the unit itself are unchanged.
  Three FDT edge-case regressions fail before and pass on the scratch-only proposal, in debug and optimized builds;
  they are not demonstrated causes of the observed Gen1 hang. The exact failed running FDT/final segment map and
  UART progress through purgatory remain the highest-value missing evidence. Coordinator owns cross-vendor unit-tier
  review and the next authorized diagnostic round; task remains active.
- 2026-11-15: Final `cargo xtask check --fast` exit 0: `partial gate passed (not done): 8 steps in 20 s`.
  Passed: fmt, size, architecture, mutable-statics, unsafe, method-exceptions, known-failures, agent-rules. All remaining
  inventory checks printed not-selected, including debug/release workspace tests, clippy, perf, coverage, host scripts
  and image checks. Complete output: `target/t0054/fast-gate.log`; per-step logs:
  `target/check-logs/80297-1794758070142397000/`. Full gate not run, as assigned. `git diff --check` passed.
  No commit, push, merge or hardware access; proposed message is `target/COMMIT_MSG.txt`.
- 2026-11-15: Codex claimed the assigned host analysis on `t0054-kexec-dt`, base
  `b10b629e76744f1eb87b6a9acf51db779dff3808`. Only the task file is reserved; no executable paths are changed.
  Independent cross-vendor unit-tier review and hardware confirmation remain coordinator work.
- 2026-11-15: seeded by the coordinator.
