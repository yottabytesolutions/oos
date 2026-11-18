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
- [ADRs](docs/adr/README.md): one flat numbered series. Rewrite an approved decision in place; Git keeps history.
  Cite records as `ADR NNNN`.
- [Tasks](docs/tasks/README.md) and [releases](docs/releases.md): claim work before starting.

## Working together

- Root AGENTS.md is the shared instruction file. Client entry files only load it; personal permissions, memory,
  local paths and unit addresses stay outside the tree. The agent-rules gate checks the entry files.

- Use one git worktree per agent and one build target directory per worktree. Never build or edit in another
  agent's worktree.
- Give subagents the task file and the paths they need, not a summary of the architecture. Keep writes disjoint
  across contributors and subagents. Reserve paths in the claim before editing them.
- Keep the task's Status current. It is shared memory across context resets and subagents.
- Report the gate's result as printed, including failed steps. Read the command's exit code, not its tail.
  Never pipe the gate or `tools/perf-guard.sh` through `tail` or `grep` inside an `&&` chain. That hides failures.
- Take commit hashes from `git log -1`. Never quote a hash from memory.
- Read [the lessons log](docs/process/lessons.md) before you start. Add an entry when you fix a mistake.
- Push your own feature branch early and often. Never push `main`. Never change a remote or its push URL.
- Land your own pull request through the merge queue once the required checks are green and Codex has no
  unresolved findings: enable auto-merge (`gh pr merge --auto --rebase`). Paths in CODEOWNERS also need the
  maintainer's approval. A stack lands bottom up
  ([stacked pull requests](CONTRIBUTING.md#stacked-pull-requests)); GitHub refuses auto-merge on stacks, so
  enqueue the top with `gh api -X PUT repos/{owner}/{repo}/pulls/<n>/merge-async` (no merge parameters: the queue
  sets the method), which takes the PRs below it along.
- On the scope, stop at the first unexpected state. Report it. Do not repair it by hand.
- No `rm -rf` outside your own worktree and scratch directories. Never force push `main`; a feature branch only
  with `--force-with-lease`. Never write partitions, run `dd` to block devices, or delete another contributor's
  worktrees or branches.
- Follow [integration order and parallel claims](docs/process/integration.md) and the [review protocol](docs/process/review.md). Use the roles defined in [integration](docs/process/integration.md#roles).
- Follow the pace in [delivery](docs/process/delivery.md): one job per task, review tier, fast gate for workers, batch integration.
- Every agent-authored change needs review by an agent from another vendor. Codex authors use Claude reviewers, and Claude authors use Codex reviewers. Other vendors pair across vendors too.
- Human-authored changes need review by any reviewer who did not write the change.
- Reviewers flag duplicated logic, near-copies and missing reuse; extend an existing abstraction, not a parallel one ([ADR 0007](docs/adr/0007-less-production-code.md)).
- History is linear: rebase onto `main`, never merge `main` into a branch; `main` only fast-forwards. Set
  `git config pull.rebase true` and `rebase.autoStash true`. Conflict resolutions after review need renewed review.
- Follow the merge, acceptance and publication order in [integration](docs/process/integration.md#review-and-merge).
- Record evidence in the task file's Status lines and the merge commit body. The PR template is the field list. Use it in the PR when one exists.
- Fix unit regressions on the unit branch, then forward-port the failing regression test plus fix through review and the full gate.

## The gate

- The gate is `cargo xtask check` ([ADR 0005](docs/adr/0005-the-gate.md)).
- Set up once per clone: `tools/bootstrap.sh` (pinned toolchain), `git config core.hooksPath .githooks`.
- Run `cargo xtask check --fast` before every commit and push (the hooks run it). Push early; CI runs the full gate.
- Report a change as done only after the required CI checks passed on its final commit.
- Never bypass a hook (`--no-verify`, `core.hooksPath` changes, editing `.githooks/`).
- Never edit a ratchet file (`xtask/data/<crate>/*.txt`, `coverage.txt`) by hand; fix the finding. A PR that lowers a
  count commits `cargo xtask ratchet` (`--move` for planned moves; [ADR 0006](docs/adr/0006-format-lints-and-ratchets.md)).
  Planned moves go in `xtask/data/moves.txt`, part of the step, not a ratchet file.
- Never loosen a lint, a limit, `rustfmt.toml`, `clippy.toml` or `xtask/data/architecture.txt` without an ADR the
  owner has approved. Adding a module rule that the tree meets is not loosening.
- No `#[allow(...)]` without a reason comment on the same item.

## Code Review Rules

- Default to one PR with separate topic commits, about 400 changed lines without goldens and fixtures; link the issue
  or task file and use the template. Multi-PR work may use a `gh stack`, one green topic per layer
  ([stacked pull requests](CONTRIBUTING.md#stacked-pull-requests)).
- Required checks: fast, clippy, test, python, deny, pr-meta, privacy, codex-review. Fix or answer every Codex
  finding; never resolve a thread without either. `@codex review` after new commits. Outages: an admin merges via
  the ruleset bypass and states the reason in the PR.
- Native Windows portability is out of scope for review findings; WSL counts as Linux ([ADR 0010](docs/adr/0010-development-platforms.md)).
- Risk tiers ([CONTRIBUTING](CONTRIBUTING.md#risk-tiers)): CODEOWNERS paths need code owner review. The hardware
  ones also need `risk/hardware`, the `plan-matrix` gate step, and a unit run (`needs-unit` until then).
  An intended register change includes a reviewed `cargo xtask plan-matrix --update` baseline.
- State in the PR whether it ran on a unit, which model and build, or host only. Never claim an untested unit run.
- Flaky tests are fixed. Ignore one only with `#[ignore = "flaky: #NNN reason"]` and an open `flaky` issue.
- Agents take only `agent-ready` issues, stay within acceptance criteria and out of scope, avoid high-risk paths
  unless the issue says so, and push only as `yottabyte-oos-agent[bot]`, never with a maintainer's credentials.

### Review guidelines

Read [Rust style](docs/process/rust-style.md) and the [domain glossary](docs/architecture/glossary.md).
Report violations of these reviewed rules as P2 findings, with a concrete location and maintainability consequence:

- Names describe the concept and effect: types are domain nouns, functions are verbs, abbreviations and glossary
  terms have their stated meaning, and the full module path reads clearly. Check generated names too.
- Types and modules have one cohesive responsibility; callers and callees use a consistent abstraction level.
  Keep visibility narrow and place code in the existing architecture module map.
- Traits represent real substitution or IO seams. Reuse and extend existing abstractions; flag duplicated logic.
- Distinct units and identifiers use newtypes; states and mutually exclusive alternatives use enums.
- Iterators express traversal clearly; borrows express access without unnecessary ownership, cloning or allocation.
- New errors are typed enums, never `Result<_, String>`; convert them to text at reporting boundaries.
- Comments explain why, constraints and invariants. Check unsafe explanations, exception reasons and production
  `expect` invariants for evidence, rather than accepting annotations alone.

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
  and keep real duration assertions in perf-guard; see [ADR 0009](docs/adr/0009-test-time.md).
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
- A new dependency has a line in `xtask/data/architecture.txt`, a replacement rationale in `docs/fork/deps.md`,
  cargo-deny evidence and maintainer approval ([ADR 0004](docs/adr/0004-dependencies.md)).
  IO stays behind ports; non-blocking IO and its synchronous exceptions follow [ADR 0023](docs/adr/0023-io-layer.md).

## Privacy

- No hostnames, LAN addresses, MAC addresses, serial numbers, licence keys, local filesystem paths or other personal
  data in code, tests, docs, fixtures or commit messages.
- Use placeholders: `example.invalid` for hosts, `192.0.2.x` for addresses, `00:00:5e:00:53:xx` for MACs,
  `/path/to/` for paths.
- The publish scrub is a backstop, not a licence. Write as if every commit were public.

## Commits

- One logical change per commit. Formatting and moves in their own commits.
- Subject `type(scope): imperative summary` (Conventional Commits), under 72 characters. The body says why, not
  what ([contribution rules](CONTRIBUTING.md#style)).
- Every commit is signed off (`git commit -s`,
  [ADR 0003](docs/adr/0003-contributions.md)) with a GitHub
  noreply address. No tool, session or agent attribution and no co-author lines in commits or PR texts.
- No em-dashes in commit messages, docs or comments.
- Pull requests merge by rebase: every commit lands on main as it is. Each one is a Conventional, signed-off topic
  commit; fold fix-ups in with `git rebase -i --autosquash` before marking the PR ready. After a review, fold a
  fix in only with renewed review. Never rebase or amend commits that someone else has merged.
- A fix the unit needs now branches from `dho804` as `fix/<name>` and is forward-ported to `main` as a test plus
  the fix in its target place ([Migration](docs/architecture/migration.md), Hotfix lane).

## Docs

- Work comes from GitHub issues labelled `agent-ready`, with acceptance criteria (CONTRIBUTING.md). A multi-step design gets
  a task file. Create or claim a numbered task before such work. Follow [the task format](docs/tasks/README.md).
  Set `status: active` and `owner`. Record the branch and reserved paths. Name the branch
  `<type>/t<NNNN>-<slug>` (`feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `ci`, `build`, `chore`; CONTRIBUTING.md).
- Serialize ids and claims on the integration branch. Do not start work on an unaccepted or overlapping claim.
  Reserve hot files explicitly. Task commands and automatic claim checks are pending task 0040.
- Write the Plan before code. Update Status after each step with dated lines, newest first.
- In the same merge that finishes the task, set `status: done`, add a dated Status line and move the file to
  `docs/tasks/done/`. Keep completed files and ids forever. Never reuse or delete an id.
- Fold lasting facts into permanent docs when the change merges.
- A new architectural decision gets the next unused number in flat `docs/adr/` (from `docs/adr/template.md`). Rewrite an existing record when its decision changes. The owner approves decisions before code.
- Update `docs/architecture/README.md` when code matches it or when the target changes.
- Hardware statements in docs and reports say verified on the unit (with model and date), assumed, or not yet verified on hardware.

## Upstream

This tree is a hard fork of OOS ([ADR 0001](docs/adr/0001-hard-fork.md)). `main` is
the trunk. Upstream is not merged or tracked. No pull requests go upstream. A single upstream change enters by
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
- Never write FPGA slots `0x000000` or `0x400000`, or send PROG for them. Current OOS writes no FPGA flash;
  a future opt-in OOS image in slot `0x800000` needs its own owner-approved record (ADR 0011).
  Never write the `p4`, `p5` or `uboot` partitions.
- Calibration files are read-only. Never write, move or regenerate them.
- Install only signed packages through `oos-update` or the web update page. No hand-copied binaries.
- After a reboot, wait for the `OOSK` re-arm before the next one. Never two reboots within 60 s.
- A wedged unit needs its power removed by the owner. Report it at once; do not retry.
- Agents never attempt automatic recovery of the unit. Stop and report unexpected state.
- No hardware access without the owner's release of the unit for your task.
