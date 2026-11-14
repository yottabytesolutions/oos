The largest gaps are coordination and enforcement: several universal rules are Claude-only, task ownership is not atomic, and access to the shared scope has no general lease.

This was a static, read-only audit. No files changed, no builds or tests ran, and no network was used. Branch-qualified paths below describe the audited snapshots, before the documentation moves. References use:

- **O**: `overhaul` at `50d48c0`
- **A**: `overhaul-agents` at `b6c7683`
- **G**: `overhaul-guardrails` at `fcb0074`
- **P**: the planned [0001-docs-and-tasks.md](../0001-docs-and-tasks.md)

“Already covered” describes verified repository content. Problems are inferred failure scenarios; proposals are additions. Actual branch protection, credentials, external coordination and hardware state were not inspected. Effort: **S** small, **M** moderate, **L** substantial.

1. **Put every universal rule in AGENTS.md. Effort: S.**  
   **Already covered: partly**: `A:AGENTS.md` declares shared rules, but worktree isolation, disjoint edits, preserving command exit codes, no remote changes, no self-merge and stopping on unexpected hardware state live in `A:CLAUDE.md`. Only AGENTS and Claude entry files exist in the inspected tree.  
   **Problem:** another vendor’s agent misses rules essential to safe collaboration.  
   **Proposal:** move those rules into AGENTS. Add thin pointers for Gemini, Copilot, Cursor and Aider, plus a documented explicit-load command for other clients; check shim contents in the gate so they cannot acquire independent policy.

2. **Make task and hot-file claims atomic. Effort: M.**  
   **Already covered: partly, planned**: P adds `owner`, `status: active`, numbered tasks and validation. `A:CLAUDE.md` prohibits overlapping subagent writes, but no equivalent reservation protocol exists between contributors.  
   **Problem:** two worktrees independently claim the same task, allocate the same “next free” ID, or edit `ui.rs`, `board/dho/hw.rs` and `model.rs` simultaneously.  
   **Proposal:** use one authoritative, vendor-neutral claim service or designated coordinator with serialized claims. Claims should include task, contributor/session, branch, base commit and reserved paths; task front matter records the accepted claim rather than acting as the lock.

3. **Lease the physical scope across all clients. Effort: M.**  
   **Already covered: partly**: `A:AGENTS.md`, “The scope,” requires owner release; `O:docs/update.md` documents an update-state lock. The inspected device tools have no general lease.  
   **Problem:** one agent changes acquisition settings or deploys while another measures, invalidating results or interrupting an operation.  
   **Proposal:** require an exclusive lease for automated access, including SCPI tests and apparently read-only observations that need stable state. Record task, holder, installed build, wiring and heartbeat; expired leases should quarantine the unit until checked, rather than automatically authorizing another client.

4. **Separate hardware release from permission to install or recover. Effort: M.**  
   **Already covered: partly**: `A:AGENTS.md` lists forbidden operations and signed-install rules. However, `G:tools/device-deploy.sh` uploads binaries to `/tmp` and kills/restarts the UI, while `O:docs/overhaul-migration.md` prescribes direct `--no-kexec` installation for unit rounds.  
   **Problem:** permission to test is interpreted as permission to replace software, reboot or attempt recovery.  
   **Proposal:** define explicit capabilities: observe, configure/test, install and reboot, each requiring task-specific authorization. Mark the deploy script owner-only or retire it, prohibit agents from automatic recovery, and add a universal destructive-command policy covering disks, worktrees, cleanup and force operations.

5. **Define integration order and branch-update policy. Effort: S.**  
   **Already covered: partly**: `O:docs/overhaul-migration.md` requires one step per branch; P waits for the two prerequisite merges. `A:AGENTS.md` prohibits rebasing reviewed commits.  
   **Problem:** overlapping migration branches pass separately but conflict semantically after integration, or reviewed work is rewritten to resolve conflicts.  
   **Proposal:** publish the dependency order and permitted parallel areas, serialize hot-file and layout migrations, and assign conflict resolution to the affected owners. Allow rebasing private unreviewed work; update reviewed branches through merges or replacement review, then rerun the gate on the integrated result.

6. **Name area reviewers and enforce merge authority. Effort: M.**  
   **Already covered: partly**: `A:CLAUDE.md` says the coordinator merges after review; AGENTS says the owner pushes. No CODEOWNERS file exists in the inspected branch trees; repository protection settings remain unknown.  
   **Problem:** merge authority varies by client, and changes to acquisition, update safety or the gate receive no responsible specialist review.  
   **Proposal:** put merge authority in AGENTS and add CODEOWNERS for acquisition/HAL, picture/GPU, interfaces, boot/update and guardrails. Require applicable human approval and final CI through repository protection; CODEOWNERS supplies review responsibility, while item 2 supplies edit reservations.

7. **Protect the gate against weakening itself. Effort: M.**  
   **Already covered: partly**: `G:xtask/src/ratchet.rs` compares code with the candidate’s current baseline; `tighten.rs` exposes `--init`. AGENTS prohibits increasing allowances, lowering coverage and loosening rules.  
   **Problem:** editing a baseline, lint level or checker alongside the code can make a regression pass. A deleted coverage file becomes an empty baseline, and missing coverage entries default to zero.  
   **Proposal:** add a protected base-versus-candidate policy check for baseline increases, missing entries, lint suppression and removed checks. Require explicit owner-approved exceptions for gate changes, and reject duplicate or missing required baseline keys.

8. **Add privacy and secret checks before commits reach publication. Effort: M.**  
   **Already covered: partly**: `A:AGENTS.md` defines privacy rules and placeholders; `G:xtask/src/check.rs` has no scanner. `G:tools/device-test.py` still has a concrete private-LAN default host.  
   **Problem:** agents copy bench addresses, identifiers, credentials or local paths into fixtures, diagnostics and commit messages.  
   **Proposal:** scan staged content locally and the incoming commit range in CI, including messages; remove the default hardware address and require explicit configuration. Use reviewed exceptions for synthetic network fixtures, public test keys and required device paths, rather than rejecting every private address or absolute path.

9. **Pin the toolchain and provide one bootstrap command. Effort: M.**  
   **Already covered: partly**: G adds a workspace lockfile and linker flags, but has no `rust-toolchain.toml`. `G:docs/development.md` installs unversioned `cargo-llvm-cov`; CI installs floating stable and an unpinned coverage tool.  
   **Problem:** contributors receive different formatting, lints, coverage and compiler behavior.  
   **Proposal:** pin the tested Rust release, components and target; pin coverage and other required tools. Provide one bootstrap/doctor command used by CI that verifies versions, linker availability, Python dependencies and optional image/GPU prerequisites.

10. **Make output-directory isolation consistent across tools. Effort: M.**  
    **Already covered: partly**: isolation is required only in `A:CLAUDE.md`. G uses worktree-local defaults, but `coverage.rs` hardcodes `target/xtask-cov`, build/deploy scripts assume root `target`, and `target.rs` does not implement the separate `target/xtask-arm64` directory described in development.md.  
    **Problem:** setting a custom target directory builds one artifact while scripts consume another, potentially stale artifact.  
    **Proposal:** define one output-directory contract and use it throughout Cargo, coverage, GPU, image and deploy tools. Reject unsupported overrides or resolve them consistently, and verify artifact revision before consumption.

11. **Isolate host harness resources, not just worktrees. Effort: M.**  
    **Already covered: partly**: `G:tools/device-test.py --sim` uses fixed ports `15555` and `18080`; `tools/ssh-sim.sh` and `tools/fw-sim.sh` delete fixed directories under the system temporary directory. `ui/src/main.rs::serve` also starts VXI-11, and `net6.rs` binds listeners on unspecified addresses.  
    **Problem:** simultaneous runs delete each other’s fixtures or accidentally test another agent’s server; simulated services are exposed beyond loopback.  
    **Proposal:** use unique run directories, loopback listeners and dynamically allocated ports, including VXI-11 or its explicit disablement. Replace the startup sleep with a readiness handshake identifying the child process and build.

12. **Land M1 golden pictures before drawing migrations. Effort: M.**  
    **Already covered: partly, planned**: `O:docs/adr/0011-golden-pictures.md` and migration M1 specify committed references. `G:ui/tools/layout-check.sh` provides manual record/check comparison, but the inspected trees contain no `ui/tests/golden/`, and the gate does not run golden comparisons.  
    **Problem:** refactors change pictures unnoticed, or references are first generated after the regression.  
    **Proposal:** capture the pre-M2 Demo and scripted-batch baselines, commit them and run comparisons in CI. Specify pixel tolerances, deterministic rendering inputs and an explicit update command; publish expected/actual/difference pictures for review.

13. **Bring the acquisition host harness forward as a concurrency prerequisite. Effort: L.**  
    **Already covered: partly, planned**: `O:docs/architecture.md` specifies Board, Device, display and clock fakes. M3.0 introduces the scripted acquisition device; M5 and M16 introduce broader contract and session tests. Demo serving and HAL Recorder tests already exist.  
    **Problem:** Demo tests pass while real acquisition sequencing breaks, forcing multiple contributors onto the single unit.  
    **Proposal:** implement the scripted Device loop and shared Board contract before parallel acquisition work. Cover STOP/SINGLE, reconfiguration during fill, roll/scan, stale generations, queue pressure and device errors so hardware becomes an acceptance resource rather than the normal development loop.

14. **Make the advertised full gate include its required non-Rust checks. Effort: M.**  
    **Already covered: partly**: `O:docs/extending.md` and migration rules include `gpu/test-host.sh`; `G:xtask/src/check.rs` does not. Python/shell syntax runs only in CI, while SSH/firewall simulations remain separate instructions in `G:docs/fork/sync-checklist.md`.  
    **Problem:** “full gate passed” means different things locally, in CI and in migration review.  
    **Proposal:** define a single check inventory and expose it through xtask, including syntax, GPU comparisons and relevant host simulations. If expensive checks use separate CI jobs, define completion as the aggregate and make omitted prerequisites visible.

15. **Link the scope binaries in required PR validation. Effort: M.**  
    **Already covered: partly**: `G:xtask/src/target.rs` runs `cargo check`, which does not link. The image workflow builds images, but runs manually, for tags, or when that workflow changes.  
    **Problem:** a normal Rust PR passes the gate while the release binary fails during linking or image assembly.  
    **Proposal:** require an ARM64 musl release build of the shipped binaries for PRs. Add image assembly validation for boot/kernel/rootfs/build changes, with final artifact and manifest evidence.

16. **Close architecture-checker blind spots. Effort: M.**  
    **Already covered: partly**: `G:xtask/src/arch/modules.rs` scans explicit `crate`, `self` and `super` paths using filesystem-derived module names. It does not resolve re-exports, macro bodies or arbitrary module mappings; `architecture.txt` currently contains no module boundary rules.  
    **Problem:** an adapter dependency passes through an allowed re-export, or newly migrated modules never receive their promised boundaries.  
    **Proposal:** add the boundary rules with each migration and check that required migrated areas have rules. Test re-export, inline-module and macro cases; constrain unsupported patterns explicitly and use crate boundaries for guarantees the syntax checker cannot reliably enforce.

17. **Ensure every workspace member enters every applicable check. Effort: S.**  
    **Already covered: partly**: `G:xtask/src/repo.rs` hardcodes five crates. `architecture.txt` already allows the coming `oos-core`, and development.md tells contributors to update CRATES manually.  
    **Problem:** adding `oos-core` to the workspace without CRATES leaves it outside direct size, lint, test and coverage enforcement.  
    **Proposal:** compare workspace metadata against the gate registry and fail on omissions or stale entries. Store only exceptional properties such as scope targeting and legacy-ratchet eligibility manually.

18. **Pin the image and GPU build environments. Effort: M.**  
    **Already covered: partly**: `G:rootfs/deps.lock` and `kernel/base.sha256` pin sources; the image workflow compares cross-host manifests. GPU scripts still use mutable Ubuntu tags and unpinned apt packages; image construction embeds the current time and excludes OOS program differences from strict comparison.  
    **Problem:** identical source produces materially different artifacts depending on build date and local Docker cache.  
    **Proposal:** pin container digests and package snapshots or publish versioned build images. Support a fixed source timestamp and record compiler/container identities, distinguishing reproducible payloads from intentionally variable package metadata.

19. **Make signing an explicit operation with controlled key selection. Effort: M.**  
    **Already covered: partly**: `O:docs/update.md` explicitly publishes the community key and documents private keys outside the repo. `G:boot/build-dev-image.sh` automatically selects a developer’s home-directory private key before falling back to the community key.  
    **Problem:** ordinary agent builds use a real signing identity implicitly; community signatures are mistaken for authorization to install on the shared scope.  
    **Proposal:** remove automatic private-key discovery and separate build/package/sign steps. If all signing keys must leave the repo, generate ephemeral test keys outside it and keep only public fixtures; otherwise explicitly classify the community key as a public development fixture and document the bench’s trust policy.

20. **Require an independent adversarial review with architectural evidence. Effort: S.**  
    **Already covered: partly**: `O:docs/overhaul.md` and migration.md call for reviewed steps, and extending.md supplies feature-specific tests. No PR template exists in the inspected trees; independence is not defined.  
    **Problem:** the implementing agent reviews its own assumptions, and green checks substitute for checking ownership and behavior.  
    **Proposal:** add a PR template naming task/step, affected ownership boundaries, failing-before evidence, deletions, golden changes and hardware evidence. Require a fresh reviewer to challenge settings writes, BatchMeta usage, generation/epoch filtering, picture ownership, blocking work and recovery behavior; a human remains accountable for acceptance.

21. **Make mandatory tests hermetic and skipped coverage explicit. Effort: M.**  
    **Already covered: partly**: `G:hal/src/dho/boot/tests.rs` optionally loads `OOS_CAL_DIR` or `local/cal`; ADC assertions are skipped inside an otherwise passing test. `testrun.rs` adds `--locked` only under CI, and the other Cargo runners do not consistently request it.  
    **Problem:** two “passing” runs exercise different behavior and dependency states without clearly reporting that difference.  
    **Proposal:** use synthetic calibration fixtures for mandatory tests and separate private-calibration validation into an explicit suite. Normalize configuration, clock and storage inputs, use locked dependency resolution everywhere, and report skipped scenarios and zero-test selections as structured outcomes.

22. **Provide bounded fast feedback and focused iteration commands. Effort: M.**  
    **Already covered: partly**: `G:xtask/src/check.rs` separates fast/full checks and times steps. Full validation serially repeats per-crate linting, debug/release tests and coverage; there is no focused xtask mode or measured timing budget.  
    **Problem:** agents repeatedly run the whole suite for a local edit, or skip validation because iteration is slow.  
    **Proposal:** measure cold/warm fast and full runs on supported hosts and publish timing budgets. Add explicit crate/test/check selection for development, reduce repeated syntax walks, and print the remaining required checks before a focused run can be called complete.

23. **Emit structured, actionable gate results. Effort: M.**  
    **Already covered: partly**: `G:xtask/src/report.rs` prints timed text; `run.rs` captures subprocess output without timeouts, and several callers return only its tail. Clippy JSON is reduced to counts rather than retaining full finding locations.  
    **Problem:** an agent cannot reliably distinguish missing prerequisites, regressions and hung commands, or locate the relevant failure.  
    **Proposal:** add versioned JSON output with check ID, status, command, location, duration, baseline and reproduction/fix instructions. Retain full per-run logs, add bounded subprocess execution, and print the checked revision, dirty state and executed check set.

24. **Add hostile-input and unsafe-code validation beyond coverage. Effort: L.**  
    **Already covered: partly**: `G:SECURITY.md` identifies network/file crashes as reportable; HAL contains in-tree update, archive and cryptographic implementations, and GPU code is C. G supplies tests, linting and coverage, but no fuzzing or sanitizer workflow appears in the inspected inventory.  
    **Problem:** high line coverage misses malformed lengths, parser state transitions and memory errors in remotely reachable or update-sensitive code.  
    **Proposal:** add bounded fuzz targets for SCPI/HTTP/setup parsing and update/package/archive readers, using synthetic corpora. Run supported host unsafe paths under Miri and C GPU tests under sanitizers in a separate required or scheduled tier, with retained reproducible failures.

25. **Resolve task/release lifecycle inconsistencies before implementing P. Effort: S.**  
    **Already covered: partly, planned**: P adds schema, dependencies, releases and link checks, but deletes completed release tasks while requiring every dependency ID to exist. `A:AGENTS.md` instead deletes task files immediately after merge.  
    **Problem:** later-release tasks lose dependency targets, agents follow conflicting completion rules, and concurrent edits make `releases.md` another hotspot.  
    **Proposal:** choose one retention rule and preserve completed IDs through archived tasks or tombstones understood by the checker. Define valid transitions and handoff records, serialize release edits, and update AGENTS and all shims in the same migration; the planned link checker should then enforce the moved documentation paths.