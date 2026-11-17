# Rules for agents and contributors

These rules apply to every change in this tree, by a human or an agent. They are short on purpose. Each rule points
to the document that explains it. When a rule and your task conflict, stop and ask the owner; do not work around it.

## Read first

- [Architecture](docs/architecture/README.md): start with [Why](docs/architecture/README.md#why),
  [Layers](docs/architecture/README.md#layers) and [Module map](docs/architecture/README.md#module-map).
- [State and ownership](docs/architecture/README.md#state-and-ownership) and
  [Enforced by visibility](docs/architecture/README.md#enforced-by-visibility): who may write what.
- [Settings and commands](docs/architecture/README.md#settings-and-commands) and
  [Change detection](docs/architecture/README.md#change-detection): how settings change and who reacts.
- [Step pipeline](docs/architecture/README.md#step-pipeline) and
  [Threads and messages](docs/architecture/README.md#threads-and-messages): order and execution.
- Read the part for your area: [acquisition](docs/architecture/acquisition.md),
  [picture](docs/architecture/picture.md), [records](docs/architecture/records.md) or
  [system](docs/architecture/system.md).
- [Extending OOS](docs/architecture/extending.md): follow the recipe for your change.
- [Development](docs/development.md): the gate and ratchets.
- [Migration](docs/architecture/migration.md): the relevant step is the specification.
- [ADRs](docs/adr/README.md) and [architecture records](docs/architecture/adr/README.md): do not reopen a decision
  without a new ADR. Cite `docs/adr/` records as `ADR NNNN`, the closed architecture series as `architecture/NNNN`.
- [Tasks](docs/tasks/README.md) and [releases](docs/releases.md): claim work before starting.

## Working together

- Use one git worktree per agent and one build target directory per worktree. Never build or edit in another
  agent's worktree.
- Give subagents the task file and the paths they need, not a summary of the architecture. Keep writes disjoint
  across contributors and subagents. Reserve paths in the claim before editing them.
- Keep the task's Status current. It is shared memory across context resets and subagents.
- Report the gate's result as printed, including failed steps. Read the command's exit code, not its tail.
  Never pipe the gate or `tools/perf-guard.sh` through `tail` or `grep` inside an `&&` chain. That hides failures.
- Take commit hashes from `git log -1`. Never quote a hash from memory.
- Only the coordinator pushes, and only with the owner's go, through the documented publish path. Other agents never push. Never change a remote or its push URL.
- Never merge your own branch. The coordinator merges after review.
- On the scope, stop at the first unexpected state. Report it. Do not repair it by hand.
- No `rm -rf` outside your own worktree and scratch directories. Never force push `main`; a feature branch only
  with `--force-with-lease`. Never write partitions, run `dd` to block devices, or delete another contributor's
  worktrees or branches.
- Follow [integration order and parallel claims](docs/process/integration.md) and the [review protocol](docs/process/review.md). Use the roles defined in [integration](docs/process/integration.md#roles).
- Follow the pace in [delivery](docs/process/delivery.md): one job per task, review tier, fast gate for workers, batch integration.
- Every agent-authored change needs review by an agent from another vendor. Codex authors use Claude reviewers, and Claude authors use Codex reviewers. Other vendors pair across vendors too.
- Human-authored changes need review by any reviewer who did not write the change.
- History is linear: rebase onto `main`, never merge `main` into a branch; `main` only fast-forwards. Set
  `git config pull.rebase true` and `rebase.autoStash true`. Conflict resolutions after review need renewed review. The author
  never merges the change ([ADR 0026](docs/adr/process/0026-contributions-are-signed-off-and-hold-no-vendor-material.md)).
- Follow the merge, acceptance and publication order in [integration](docs/process/integration.md#review-and-merge).
- Record evidence in the task file's Status lines and the merge commit body. The PR template is the field list. Use it in the PR when one exists.
- Fix unit regressions on the unit branch, then forward-port the failing regression test plus fix through review and the full gate.

## The gate

- The gate is `cargo xtask check` ([ADR 0031](docs/adr/process/0031-the-gate-is-cargo-xtask-check.md)).
- Set up once per clone: `tools/bootstrap.sh` (pinned toolchain), `git config core.hooksPath .githooks`.
- Run `cargo xtask check --fast` before every commit and push (the hooks run it). Push early; CI runs the full gate.
- Report a change as done only after the required CI checks passed on its final commit.
- Never bypass a hook (`--no-verify`, `core.hooksPath` changes, editing `.githooks/`).
- Never edit a ratchet file (`xtask/data/size.txt`, `clippy.txt`, `statics.txt`, `unsafe.txt`, `coverage.txt`). Fix
  the finding. The integrator runs `cargo xtask ratchet` (and `--move` for planned moves) at merge. A step that moves
  code lists its moves in `xtask/data/moves.txt`; that file is part of the step, not a ratchet file.
- Never loosen a lint, a limit, `rustfmt.toml`, `clippy.toml` or `xtask/data/architecture.txt` without an ADR the
  owner has approved. Adding a module rule that the tree meets is not loosening.
- No `#[allow(...)]` without a reason comment on the same item.

## Code Review Rules

- One issue, one branch, one PR. Keep it small (about 400 changed lines without goldens and fixtures); split
  refactors into stacked PRs. Link the issue (`Closes #NNN`) or task file and fill in the template.
- Required checks: fast, clippy, test, python, deny, pr-meta, privacy, codex-review. Fix or answer every Codex
  finding; never resolve a thread without either. `@codex review` after new commits. Outages: an admin merges via
  the ruleset bypass and states the reason in the PR.
- Risk tiers ([CONTRIBUTING](CONTRIBUTING.md#risk-tiers)): CODEOWNERS paths need code owner review. The hardware
  ones also need `risk/hardware`, `tools/plan-matrix.sh` before and after, and a unit run (`needs-unit` until then).
- State in the PR whether it ran on a unit, which model and build, or host only. Never claim an untested unit run.
- Flaky tests are fixed. Ignore one only with `#[ignore = "flaky: #NNN reason"]` and an open `flaky` issue.
- Agents take only `agent-ready` issues, stay within acceptance criteria and out of scope, avoid high-risk paths
  unless the issue says so, and push only as `yottabyte-oos-agent[bot]`, never with a maintainer's credentials.

## Where code goes

- Find the module in [Module map](docs/architecture/README.md#module-map). If nothing fits, ask; do not create a new
  top-level module on your own.
- The domain lives in the crate `oos-core`: settings, commands, change detection, acquisition data types, the picture
  model, `Config`. It does no IO, starts no thread, reads no clock and holds no global state.
- Adapters (`board/dho`, `display`, `input`, `remote`, `store`, `system`, `instruments`, `protocols`) do not use each
  other.
- Only `board/dho` uses `oos_hal::dho`. Only `display` uses DRM, the GPU helper and the VPU.

## Ownership

- Settings change only through commands applied by the change stage. Menus, views, input and remote code return
  commands; they never write settings.
- Internal writers (autoset, Bode, SINGLE done) put commands into the outbox for the next step.
- Owned types have no `pub` fields.
- No new `static` with interior mutability. Metrics counters only, listed in `xtask/data/architecture.txt`.
- Only the accumulator writes the picture (`ph`, `phz`, W, the window). Everything else reads `&Picture`.
- Frames convert from their own `BatchMeta`, never from current settings.
- A new setting gets a class in `classify`; it does not compile otherwise. Do not add it to `Unrelated` to make it
  compile unless it really changes nothing that holds state.

## Tests

- No test may pass or fail depending on machine speed. Inject elapsed time, wait on completion events,
  and keep real duration assertions in perf-guard; see [architecture/0019](docs/architecture/adr/0019-test-time.md).
- Every change comes with tests. The gate's coverage floor must hold.
- A bug fix comes with a test that fails before the fix and passes after.
- A change to what is drawn comes with golden pictures (`ui/tests/golden/`). A changed reference needs a reason in
  the commit message.
- Tests pin behaviour, not internals. Use the fakes next to each port (`board::fake`, `display::fake`, ...).
- Run tests in debug and release; the gate does both.

## Code

- Idiomatic Rust. Enums for state, traits at boundaries, value semantics where they fit.
- Small units: at most 400 lines per file, 60 per function, 12 fields per struct, a type's `impl` blocks in at most
  3 files. The gate enforces it.
- No exemptions from the limits by marker or kind. A record over 12 fields groups its fields into sub-structs.
- No god objects. A new struct that collects unrelated state is wrong even under the limits.
- No `unwrap` outside tests. `expect` only with a reason that states the invariant. Errors go back to their sender.
- Locks through `oos_hal::sync::lock`.
- No commented-out code, no TODO without context, no dead code.
- `unsafe` only in `oos-hal`, the adapters (`board/dho`, `display`, `input`, `remote`, `system`, `instruments`) and the
  SIMD kernels in `picture/deposit` (NEON intrinsics, `align_to`). Never in `oos-core` or the other stages. Every
  `unsafe` block or fn has a `// SAFETY:` comment that states the invariant it relies on. The gate enforces the
  placement (`unsafe` lines in `xtask/data/architecture.txt`); the `// SAFETY:` comments are reviewed
  ([Enforcement](docs/architecture/README.md#enforcement)).
- No new dependency without a line in `xtask/data/architecture.txt` and the owner's agreement. What runs on the
  scope depends on `libc` only; `xtask` may use its host-only crates (ADR 0013).

## Privacy

- No hostnames, LAN addresses, MAC addresses, serial numbers, licence keys, local filesystem paths or other personal
  data in code, tests, docs, fixtures or commit messages.
- Use placeholders: `example.invalid` for hosts, `192.0.2.x` for addresses, `00:00:5e:00:53:xx` for MACs,
  `/path/to/` for paths.
- The publish scrub is a backstop, not a licence. Write as if every commit were public.

## Commits

- One logical change per commit. Formatting and moves in their own commits.
- Subject `type(scope): imperative summary` (Conventional Commits), under 72 characters. The body says why, not
  what ([ADR 0019](docs/adr/process/0019-one-style-for-text-files-and-commits.md)).
- Every commit is signed off (`git commit -s`,
  [ADR 0026](docs/adr/process/0026-contributions-are-signed-off-and-hold-no-vendor-material.md)) with a GitHub
  noreply address. No tool, session or agent attribution and no co-author lines in commits or PR texts.
- No em-dashes in commit messages, docs or comments.
- Never rebase or amend commits that someone else has reviewed or merged.
- A fix the unit needs now branches from `dho804` as `fix/<name>` and is forward-ported to `main` as a test plus
  the fix in its target place ([Migration](docs/architecture/migration.md), Hotfix lane).
- Only the coordinator pushes, and only with the owner's go, through the documented publish path. Other agents never push. Never change a remote or its push URL.

## Docs

- Work comes from GitHub issues labelled `agent-ready`, with acceptance criteria (ADR 0026). A multi-step design gets
  a task file. Create or claim a numbered task before such work. Follow [the task format](docs/tasks/README.md).
  Set `status: active` and `owner`. Record the branch and reserved paths. Name the branch
  `<type>/t<NNNN>-<slug>` (`feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `ci`, `build`, `chore`; ADR 0026).
- Serialize ids and claims on the integration branch. Do not start work on an unaccepted or overlapping claim.
  Reserve hot files explicitly. Task commands and automatic claim checks are pending task 0040.
- Write the Plan before code. Update Status after each step with dated lines, newest first.
- In the same merge that finishes the task, set `status: done`, add a dated Status line and move the file to
  `docs/tasks/done/`. Keep completed files and ids forever. Never reuse or delete an id.
- Fold lasting facts into permanent docs when the change merges.
- Any architectural decision gets an ADR in `docs/adr/<category>/` (from `docs/adr/template.md`, next number from
  0030).
  The owner approves it before code.
- Update `docs/architecture/README.md` when code matches it or when the target changes.
- Statements about hardware say how they are known: verified on the unit, assumed, or not yet verified.

## Upstream

This tree is a hard fork of OOS ([ADR 0030](docs/adr/process/0030-this-tree-is-a-hard-fork-of-oos.md)). `main` is
the trunk. Upstream is not merged and gets no pull requests unless the owner asks. A single upstream change enters by
cherry-pick: it keeps its author, carries `Cherry-picked-from: upstream <hash>` and passes the gate like any change.

## The scope

The unit is real hardware and can be bricked. Rules for anything that runs on it:

- Take the device lease with `tools/scope-lease` before OOS access. Set `--host` or `OOS_SCOPE`; there is no default.
  Stock adb survey and platform probe need no SSH lease. The owner's release still applies to our shared unit.
- Lease tools need SSH enabled on the unit with a trusted key. `remote.ssh` is off by default.
- Use the capability you need: `observe`, `test`, `install` or `reboot`, in that order.
- Renew at least every two minutes while using the unit. Release the lease at the end.
- An expired lease quarantines the unit. Never break a lease you do not own without the owner.
- Install and reboot need the owner's go for that task. Record it with `--go TASK` when acquiring the lease.
  An agent never issues a go for its own task. The coordinator relays the owner's go.
  Permission to test does not grant permission to install or reboot.
- No hand writes to FPGA registers while `oos-ui` runs. No reads of register `0x205c`.
- Never write the FPGA flash. Never write slot `0x000000`. Never write the `p4`, `p5` or `uboot`
  partitions.
- Calibration files are read-only. Never write, move or regenerate them.
- Install only signed packages through `oos-update` or the web update page. No hand-copied binaries.
- After a reboot, wait for the `OOSK` re-arm before the next one. Never two reboots within 60 s.
- A wedged unit needs its power removed by the owner. Report it at once; do not retry.
- Agents never attempt automatic recovery of the unit. Stop and report unexpected state.
- No hardware access without the owner's release of the unit for your task.
