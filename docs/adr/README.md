# Architecture decision records

An ADR records a structural decision with a real alternative and a cost to reverse. Style, procedures and plans live in
CONTRIBUTING.md, docs/development.md, docs/process/ and [the migration plan](../architecture/migration.md). This is one
flat series. Every number has one file. Deleted records and former numbers remain in Git history; the index lists
existing records only.

Each record has Number, Category, State and Date, then Context, Decision, Alternatives and Consequences. Categories are
policy, hardware, system and code. State is `in force` or `target: <migration step>`. A target names its follow-up in
the migration plan and distinguishes the rules already in force. Date is the date of the current owner-approved
decision. A Check line names the enforcing gate step, test, ruleset or review only; a proposed check is explicitly
marked as a target. Backticks in Check paragraphs identify executable step IDs from the inventory; CI check names and
individual test names use plain text.

The owner approves decisions before implementation. Rewrite a record in place when its decision changes; Git keeps
history. Do not append amendments, removal logs, progress counts or implementation plans. Hardware verification marks
remain part of the record. A retired decision is deleted in the change that replaces it. Contribution instructions
cannot override an owner-approved decision; report conflicts to the owner.

New decisions take the number after the highest number issued in this series. Retired numbers are never reused. Cite
them as `ADR NNNN` and link the file. The `text` gate rejects duplicate numbers, unresolved number citations and broken
ADR paths across the repository. The `links` step also checks Markdown links and anchors.

## Index

| Number | Title | Category | State |
|---|---|---|---|
| [0001](0001-hard-fork.md) | Hard fork; no upstream merges or pull requests | policy | in force |
| [0002](0002-no-vendor-material.md) | No vendor material in the tree | policy | in force |
| [0003](0003-contributions.md) | Contributions preserve authorship, privacy and linear history | policy | in force |
| [0004](0004-dependencies.md) | Dependencies are listed, licence-checked and owner-approved | policy | in force |
| [0005](0005-the-gate.md) | The gate is cargo xtask check | policy | in force |
| [0006](0006-format-lints-and-ratchets.md) | Format, limits, unsafe and ratchets protect the code | policy | target: gate tightening follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0007](0007-less-production-code.md) | Each release has less production code | policy | in force |
| [0008](0008-behaviour-baselines.md) | Behaviour-preserving changes prove equality against baselines | policy | target: exact CPU goldens and layout coverage; baseline follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0009](0009-test-time.md) | Tests do not depend on machine speed | policy | in force |
| [0010](0010-development-platforms.md) | Development supports Linux and macOS; BSDs are best effort; Windows uses WSL or a VM | policy | in force |
| [0011](0011-protected-areas.md) | OOS writes only named card areas and protects stock FPGA slots | hardware | target: protected-area evidence follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0012](0012-hardware-verification.md) | Unverified hardware functions are off by default | hardware | target: system.conf-only switches and off-until-verified defaults; hardware-key inventory follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0013](0013-recovery.md) | OOS lives in recovery and stock stays bootable | system | in force |
| [0014](0014-kernel-and-device-tree.md) | OOS ships its kernel and device tree inside recovery | system | in force |
| [0015](0015-generic-images.md) | One generic image per model keeps device state in oos | system | target: complete device-state manifest; device-state follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0016](0016-signed-updates.md) | Updates trust only the project key and use trial rollback | system | in force |
| [0017](0017-safe-start.md) | Safe start breaks restart loops | system | in force |
| [0018](0018-bench-lan-services.md) | Network services are for a bench LAN; SSH is the shell | system | target: telnet removal (#139), service switches and ACL completion; M15b and service-policy follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0019](0019-instrument-protocols.md) | Serve raw SCPI, VXI-11 and HiSLIP with live discovery | system | in force |
| [0020](0020-crates.md) | Crates have one dependency direction | code | in force |
| [0021](0021-static-programs.md) | Scope programs are static musl binaries and abort on panic | code | in force |
| [0022](0022-frame-state.md) | The frame thread owns UI state without locks | code | in force |
| [0023](0023-io-layer.md) | Non-blocking IO stays behind ports in one IO layer | code | target: IO-port follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0024](0024-scpi-interpreter.md) | One SCPI interpreter runs on the frame thread | code | in force |
| [0025](0025-scpi-identity.md) | SCPI follows stock names and identifies as OOS | code | target: SCPI conformance follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0026](0026-gpu-helper.md) | GPU drawing uses a helper and the CPU path stays complete | code | in force |
| [0027](0027-desktop-view.md) | The desktop view is the browser UI | code | in force |
| [0028](0028-registered-modules.md) | Extensions use static registered modules | code | target: registry coverage follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0029](0029-table-generation.md) | One table can generate per-field code | code | in force |
| [0030](0030-typed-errors.md) | Errors return to their sender and use typed values | code | target: typed-error follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0031](0031-performance.md) | Performance work is bounded, event-driven and measured | code | target: M3-M12 and performance-gate follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0032](0032-settings-owner.md) | Settings have one owner and change only through commands | code | target: M13, M14 |
| [0033](0033-change-point.md) | One change point classifies settings and requests | code | target: M4, M13, M14 |
| [0034](0034-step-pipeline.md) | The step is a fixed pipeline of stages | code | target: M16 |
| [0035](0035-picture-owner.md) | One accumulator owns the picture and adds whole batches | code | target: M9 and host Batch-default alignment in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0036](0036-threads-and-channels.md) | Threads share named cells and bounded channels | code | target: M5 |
| [0037](0037-ports.md) | Ports cover the IO the step depends on | code | target: M16 and IO-port follow-up in docs/architecture/migration.md#adr-baseline-follow-ups |
| [0038](0038-menus-and-parser.md) | Menus are data and the SCPI parser comes from one table | code | target: M5, M15, M17 |
