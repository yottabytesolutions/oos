# Development: the gate

Workers run the fast gate and focused tests. The coordinator runs the default full gate once per integration batch:

```sh
cargo xtask check                  # default full gate, Docker-free
cargo xtask check --fast           # partial: formatting, syntax rules, architecture, agent rules
cargo xtask check --no-coverage    # partial: the full host gate without coverage
cargo xtask check --list           # inventory only, does not run or pass the gate
cargo xtask check --image          # default steps plus Docker/emulation/image suites
cargo xtask step clippy tests-debug # partial: only the named inventory ids (CI runs the gate as parallel jobs)
```

Until 0.2.0, repository CI is off and local evidence is authoritative. The coordinator runs
`--image` before unit rounds and releases. Before unit rounds or M-step acceptance, the
[delivery rule](process/delivery.md#publication-and-unit-rounds) also requires the M1 extended host
suites and logs in the task Status. Completion requires independent review and the full
gate on the final integrated candidate; `--fast`, `--no-coverage` and `--list` are partial feedback.

Run from the repository root. Each executed step reports its time and failures, and a failed step
makes the command exit nonzero. A successful full run names its tier: `gate passed (default)`
or `gate passed (image)`. Hooks use the same command. Missing Docker cannot fail the default gate.

## Setup

Install Git, rustup and Python >=3.12 as `python3` on macOS arm64 or Linux x86-64.
Other Linux, BSD and macOS hosts are supported but not verified; on Windows use WSL 2 or a Linux VM (ADR 0033).
Doctor accepts newer Python versions and reports the interpreter actually used.
The default gate needs the scripts' declared shells (including `mksh` for Android GEL syntax),
coverage tools and the pinned Rust host/ARM toolchain. Syntax checking does not run the scripts.
Only the explicit `--image` tier needs Docker with arm64 execution, QEMU, kernel source and the
locked GPU/simulation/image profiles. Prepare it with `tools/bootstrap.sh --image`.
Missing prerequisites fail in the tier that requires them.

Host scripts are POSIX `sh` and run on Linux, the BSDs and macOS. Where the systems differ (SHA-256 tools, file
sizes) they source `tools/lib/portable.sh`. `python3 -I tools/ci/host_scripts.py check` runs shellcheck as `sh` on
every script that runs on a host; the PR fast job runs it. Scripts that run only on the scope or inside a Linux
container are not host scripts (`SCOPE` in that file).

Then run:

```sh
# Manual prerequisite if rustup cannot start xtask or bootstrap:
rustup toolchain install 1.99.1 --profile minimal --component rustfmt,clippy,llvm-tools-preview --target aarch64-unknown-linux-musl
tools/bootstrap.sh
# Explicit optional container preparation:
tools/bootstrap.sh --gpu
tools/bootstrap.sh --sim
tools/bootstrap.sh --image
cargo xtask doctor
cargo xtask doctor --image
```

`rust-toolchain.toml` is authoritative for Rust 1.99.1, components and musl std. `tools/build-tools.json` requires
Python >=3.12 and pins cargo-llvm-cov 0.9.1; bootstrap installs coverage with `--version =0.9.1 --locked` and fetches
locked Cargo dependencies. The numbered distribution still needs a real setup run on this macOS arm64 host. Linux x86-64 runner
validation is unavailable here and explicitly untested, without blocking this release scope.
Do not replace a missing distribution/component with floating stable. An explicit override can defeat the file;
doctor rejects it even if that compiler prints the same release number.

Doctor installs nothing, pulls no images, changes no hooks, reads no keys and accesses no hardware. It reports
all independent base failures, expected/actual values and setup commands without printing private paths or tool
stderr. The default checks Rust/Cargo and override identity, components, musl std, matching rustfmt/clippy,
coverage and sysroot LLVM tools, the sysroot host `rust-lld` (no PATH lld required), Python version/stdlib,
Git/base ref, hooks, lockfile and writable isolated output. A missing or mismatched
`core.hooksPath` reports a warning without failing doctor (task 0025). Linker presence does not
prove release linking (0027).
`--gpu` adds Docker and locked software EGL/GLES execution on the Docker host platform;
`--sim` adds initramfs and SSH/firewall images, NET_ADMIN/nftables/IPv6 and host
`qemu-system-aarch64`. QEMU is checked only in the sim profile. `--image` selects all profiles
plus the compiler/rootfs/simulation roles and checked kernel source. Unselected profiles are
reported as not checked. Temporary probes remove
their containers with `--rm`, use no network and keep output scratch private. Missing selected prerequisites fail.

The default gate runs `doctor-base` without Docker, in addition to committed pin/lock syntax
alongside formatting. The image tier adds the GPU/simulation doctor profiles before their consumers.
Fast checks preserve G1 and task 0020 dispatch and need Python with `tomllib`.

Install Docker and QEMU separately, without using bootstrap as an OS package installer. On x86-64, configure arm64
emulation manually using the distribution's QEMU/binfmt setup.
Pinning the binfmt installer identity belongs to task 0033 (0.3.0). Neither doctor nor bootstrap executes
registration, sudo or a privileged container.

### Container bases, recipes and provenance

`tools/container-images.json` records Ubuntu 26.04 and Alpine 3.22 per-platform manifest digests for linux/arm64
and linux/amd64, resolved by the coordinator using `docker buildx imagetools inspect` on 2026-11-15. Index digests
retain the resolution evidence; builds use the platform manifest digest. Each role also has a recipe SHA-256
and a package request list. Kernel amd64 additionally requests the cross compiler packages.

`tools/build-env.sh prepare ROLE` explicitly pulls the platform-specific base into the local Docker store,
then builds from that digest and the requested packages. Each `rootfs-<tool>` role requests only that builder's
packages. Simulation package scripts run normally; only the linux-lts installation skips scripts.
`resolve ROLE` checks local base digest/layer ancestry, platform and recipe/request contract label, returning
an immutable local image ID. Local tags are aliases only. Missing package version locks never reject setup:
apt/apk resolve the requested packages from their repositories. A cached role keeps its installed packages
until explicitly rebuilt; pinning the base and recipe does not freeze package versions or reproduce a payload.

Builders record the actual image's full installed package list, sorted as `name=version`, beside their outputs:
`kernel/out/base/build-packages.txt`, the GPU bundle's sibling `build-packages.txt`, GPU comparison output's
`build-packages.txt`, image output's `initramfs.packages.txt`, hardware helper's `oos-hw.packages.txt`, and
`rootfs/out/<artifact>.packages.txt` (`wlan-tools.packages.txt` covers both WLAN binaries).
The shared command is `tools/build-env.sh provenance ROLE --image sha256:ID --output FILE`.
These lists are informational provenance, with no expected-version comparison. They describe the builder image,
including transitive/base packages, rather than the packages in the assembled scope filesystem.

Task 0033 (0.3.0) owns complete per-role package manifest locks and the binfmt installer identity, together with
retained package sources, source timestamps and end-to-end payload reproducibility. Real builder runs on this
macOS arm64 host remain untested in the sandbox; Linux x86-64 validation is also untested and is not a hold here.

- Hooks, once per clone: `git config core.hooksPath .githooks`. pre-commit and pre-push run `check --fast`. The
  full gate runs in CI as the required checks (ADR 0031); run it locally for an early signal.
- The ratchet files are compared with the base branch `main` (or `origin/main`): the clone needs it
  (`git fetch origin main`).
- Every cargo call that resolves dependencies uses `--locked` (the `cargo xtask` alias, clippy, tests, metadata,
  coverage and `tools/perf-guard.sh`): a change of a dependency comes with its `Cargo.lock` change.

## Executable inventory and completion evidence

`xtask/src/inventory.rs` is the ordered executable registry. `cargo xtask check --list` prints its
stable IDs, commands, prerequisites and selection. The seven G1 fast steps retain their order,
followed by task 0020's agent rules. Static tool/container pin checks remain part of `fmt`.
The existing workspace registry must still cover every Cargo member. Ratchets, planned moves
and the coverage margin of 0.5 percentage points are unchanged.

| ID | Command or behavior | Prerequisite | Selection |
|---|---|---|---|
| `fmt` | format each crate and validate static pins | pinned tools | fast and full |
| `size` | syntax and move-aware size ratchets | workspace metadata | fast and full |
| `architecture` | workspace completeness and crate/module rules | Cargo.lock | fast and full |
| `mutable-statics` | syntax and per-crate ratchets | sources | fast and full |
| `unsafe` | syntax and per-crate ratchets | sources | fast and full |
| `method-exceptions` | disallowed-method exceptions | sources | fast and full |
| `known-failures` | known failures and open tasks | sources | fast and full |
| `agent-rules` | exact loaders and Git inventory | root AGENTS.md | fast and full |
| `test-time` | test sleep/deadline syntax and nonincreasing ratchet | sources | fast and full |
| `doctor-base` | base doctor probes, including active toolchain identity | bootstrap; no Docker | full |
| `doctor` | `--gpu --sim` doctor profiles | base doctor, bootstrap, Docker | `--image` |
| `clippy` | host and ARM clippy per crate | musl std and clippy | full |
| `tests-debug` | Rust tests per crate | pinned toolchain | full |
| `tests-release` | release Rust tests per crate | pinned toolchain | full |
| `perf` | `sh tools/perf-guard.sh` | toolchain | full |
| `coverage` | `cargo llvm-cov` per crate | llvm-tools, cargo-llvm-cov | full; omitted only by partial mode |
| `script-syntax` | `python3 -I tools/script_syntax.py` | Git, Python, declared shells | full |
| `python-tests` | receipted UI build; isolated unittest discovery in tools/tests, tools/privacy and ui/tools | host toolchain, local sockets | full |
| `arm64-release-link` | receipted release build with `--bins -p oos-ui -p oos-fpgad --target aarch64-unknown-linux-musl` | musl std, rust-lld | `--image` |
| `gpu-host` | `sh gpu/test-host.sh` | ARM link and gpu-test profile | `--image` |
| `ssh-sim` | prepare initramfs/rootfs and nft/Dropbear/SFTP; `sh tools/ssh-sim.sh` | simulation profile | `--image` |
| `firewall-sim` | prepare initramfs/rootfs and nft; `sh tools/fw-sim.sh` | simulation profile | `--image` |
| `image-required` | `python3 -I tools/image_required.py` | authoritative Git base | `--image` |
| `image-suite` | `python3 -I tools/image_suite.py` | checksum-pinned kernel source, image profile, QEMU | `--image` |

Script discovery uses Git's NUL-separated tracked and nonignored candidate paths, recursively,
including extensionless shebangs, hooks and rootfs entry scripts. Python is compiled without
execution, with bytecode under the selected target. Shell fragments have explicit interpreter
mappings; executable C sources and the kernel's recorded Git answers are classified as data.
Unknown executable scripts fail. Python selections must be nonzero, their executed counts must
match discovery, and every skip reason is reported with the skipped count. Git-listed `test_*.py`
files outside the declared discovery roots fail the check. Host socket cases use synthetic fixtures and
the freshly receipted UI. No default-target fallback is accepted.

The ARM link verifies all three Rust executables (`oos-ui`, `oos-fpgad`, `oos-update`), their receipts,
and static AArch64 ELF headers. GPU comparisons require exactly the pictures named by every
`ui/tests/*.txt` script on both renderers, preserve differences under the selected target, and
check all three monitor modes. SSH/firewall use a prepared builder's BusyBox, without a kernel build.
Missing tools, fixtures, receipts or outputs fail. Missing Docker fails the image tier; the default
gate never probes Docker or the kernel source.

Every check run retains complete captured command output and step results in a unique
`check-logs/` directory under the selected `CARGO_TARGET_DIR`. The report shows failed, missing
prerequisite and not-selected checks. Exit status decides success, including QEMU and fixture
commands that print PASS before failing. Keep these logs with review evidence.

The image suite checks the pinned kernel source, builds `kernel/build-base.sh`, assembles with
`boot/build-dev-image.sh`, signs explicitly with `boot/sign-dev-image.sh --community`, and packages
with `python3 -I boot/build-gel.py`. It validates boot sections/ID, DTB, cpio contents, checksums,
receipts, update signature/payload and GEL contents, including the shipped `oos-hw` and GPU bundle.
It runs unit-DTB, GEL, USB and WLAN simulations. It uses synthetic inputs and worktree-local
builder caches; it performs no install, device access or private signing-key discovery.

The `image-required` ID is retained in the image tier as advisory change classification. It compares
with the protected `main` merge base plus working-tree inputs and examines both rename paths
and deletions. Missing bases and unclassified inputs fail closed in that tier. It never runs in
the default gate or changes that gate's completion result. The coordinator runs `--image` before
unit rounds and releases regardless of classification. Workflow files retain their base versions;
no CI aggregate or matrix result is required while repository CI is off.

The gate runs its checks at the same time, each as soon as its prerequisites passed, and prints each result when it
ends; the closing summary keeps the inventory order. The perf guards and the image checks run afterwards, one at a
time (`inventory::alone`): timing ratios and emulator time limits need a quiet machine. On a warm cache with
cargo-nextest, the default gate takes about 5 minutes on a 16-core laptop, bound by the golden pictures and coverage
of `ui`; the fast gate takes a few seconds. The image tier can take hours on cold builders or emulated hosts. Retain
actual timings and exit codes with coordinator evidence.

## Test time and completion

[ADR 0019](architecture/adr/0019-test-time.md) requires tests to advance elapsed time explicitly and await
worker completion events. Real duration assertions belong to the explicit perf-guard suite. A timeout in
ordinary tests is only a named hang guard of at least 60 seconds; the shared host helper uses 120 seconds.

The `test-time` fast step follows every Cargo target and test module, including compound test-only cfgs
and cfg(test) statements or expressions inside production functions. It flags sleeps (including renamed
imports and macro bodies), clock comparisons inside or outside loops, bound elapsed/now values, real duration
assertions, and timeout calls shorter than the shared 120-second hang guard. Test-only clock reads inside
production behavior branches are also rejected. Tests of elapsed behavior inject their clock at construction
or use the same implementation through an explicit clock boundary.

Only named functions implementing hang guards, ignored manual observation tools and ignored performance
probes are exempt, in `xtask/src/test_time.rs`. Timestamp provenance checks compare against the feed origin,
without real duration budgets. Production and test builds use the same buffered worker receiver and the same
channel capacity. Test fixtures await the completion, retaining its value for the normal poll.

`xtask/data/<crate>/test-time.txt` is a store kind alongside the other ratchets. Its keys use
`time <file> <function> <finding>`. `cargo xtask ratchet` tightens it; `--move` applies `moves.txt` to it.
The protected-base comparison rejects additions or increases. Do not add new entries. The check is syntactic:
it does not infer time through arbitrary helper APIs or every possible macro DSL. Review must check those
boundaries too. Real duration budgets run only in ignored release `perf_guard` cases with `OOS_PERF_TESTS=1`.
Task 0088 records the audit and validation limits.

## CI

GitHub-hosted runners run the inventory as parallel jobs, each one `cargo xtask step <id>...` (ADR 0031):

- `.github/workflows/pr.yml` on every pull request and merge-queue group: the required checks `fast`, `clippy`,
  `test` (debug and release), `python` and `deny`. `tools/ci/changes.py` lets a job skip its build when nothing it
  depends on changed: documents, `.github/` (except `pr.yml` and files a Rust test includes) and `tools/ci/` skip
  clippy, the tests and cargo-deny. A merge-queue group whose tree equals its PR head skips the checks that passed
  on that head. Skipped jobs still report success under their required name.
- `pr.yml` runs the tests in three parallel shards per profile (`OOS_TEST_PARTITION=K/N`, nextest
  `--partition hash:K/N`; the doctests run in shard 1). A shard's skipped count includes the other shards' tests.
  Pull requests run the debug tests only; the merge queue adds the release tests. The required `test` check sums
  up every shard.
- The release tests in `pr.yml` run without LTO (`CARGO_PROFILE_RELEASE_LTO=false`, 16 codegen units): the same
  optimisation level and no debug assertions, without a fat-LTO link per test binary. CI builds use
  `CARGO_PROFILE_DEV_DEBUG=line-tables-only`.
- The test and coverage steps run each crate's tests with cargo-nextest when it is installed: every test in its own
  process, all binaries in parallel, and the doctests with `cargo test --doc` (`cargo llvm-cov nextest` for
  coverage, which measures the same lines). CI sets `OOS_TEST_RUNNER=nextest`, which requires it. Without nextest
  they run `cargo test`. The counts differ slightly: `cargo test` also counts the summary lines of tests that run
  their own binary again.
  `.config/nextest.toml` reports a test as slow after 60 s and ends it after 10 minutes (a hang guard); the step lists
  every test that ran 60 s or longer. A test that leaves a process holding its output fails. Tests kill a process
  group with `kill -KILL -- -<pgid>`: without `--`, the procps-ng `kill` of Ubuntu 24.04 signals the wrong group, or
  every process of the user (#23).
- A push to `main` runs `pr.yml` as the only writer of the Rust caches (one per job kind); its test jobs build the
  tests without running them, because the merge queue already ran them on that tree.
- `.github/workflows/main.yml`, nightly and by hand: the whole default gate in parallel jobs, including the release
  tests with LTO, coverage and the base doctor, plus cargo-deny with the day's advisories and the image.
- The image job downloads the kernel base from the pre-release `kernel-base-<version>`
  (`tools/ci/kernel-base.sh download`); `tools/ci/kernel-base.sh publish` creates it for a new `kernel/base.sha256`.
- Each run ends with a `timings` job: job and step durations in the run summary (`tools/ci/run_summary.py`).

## Privacy checks

Before committing, run `python3 tools/privacy/scan.py --staged` and
`python3 tools/privacy/scan.py --message target/COMMIT_MSG.txt`. The stdlib scanner checks the index and commit message;
the incoming-commit scanner can check additions and messages locally while Actions are disabled.
The Privacy workflow, when enabled, checks every incoming commit. Findings redact values.
Use documentation addresses and placeholders in examples, and configure device tools explicitly with `--host`
or `OOS_SCOPE` where supported. Never infer a real unit address from a checked-in example.

Keep private strings only in the ignored `local/privacy-literals.txt`, one exact value per line. Reviewed synthetic
exceptions key the exact path, rule and matched-value hash in `tools/privacy/allowlist.json`. See [the scanner instructions](../tools/privacy/README.md)
for commands, limitations, the policy format and the pending hook integration. The xtask hook point waits for
G1/task 0043; this check does not replace the full gate.

## Loading the shared agent rules

Root `AGENTS.md` is the single source of repository rules. The `agent rules` step runs after `known failures`
in both fast and full checks, including the hooks. It checks these complete loader files:

| Entry file | Permitted content |
|---|---|
| `CLAUDE.md` | the root rules pointer, optionally followed by the exact prose instruction to use the harness-provided `Claude-Session` trailer and session URL |
| `GEMINI.md` | the root rules pointer |
| `.github/copilot-instructions.md` | the root rules pointer through `../AGENTS.md` |
| `.cursor/rules/oos.mdc` | the root rules pointer through `../../AGENTS.md`, with exactly `description: OOS repository rules` and `alwaysApply: true` loader metadata |
| `.aider.conf.yml` | only `read:` followed by the single list item `  - AGENTS.md` |

The check compares complete templates, normalizing only CRLF and the presence of one final newline. Extra rules,
comments, loader keys and actual Claude session identifiers fail. Required files must be readable regular files;
symlinks in their paths fail. The relative Markdown links must resolve to the existing, nonempty root `AGENTS.md`.
Template mismatches report the first differing line without echoing file content. There is no allowance or ratchet
for this check.

Git supplies tracked and nonignored new paths with `ls-files -z --cached --others --exclude-standard`.
Additional recognized entry points fail until a reviewed inventory change supports them: `AGENTS.md`, `CLAUDE.md`,
`GEMINI.md`, `CLAUDE.local.md`, `.aider.conf.yml` and `.cursorrules` at any depth, files under any `.claude/`,
`.gemini/`, `.cursor/rules/`, `.github/prompts/`, `.github/agents/` or `.github/chatmodes/`, and Copilot
`copilot-instructions.md` files under `.github/` or `*.instructions.md` under `.github/instructions/`, including
nested directory layouts. Discovery is case-insensitive; supported entries retain their canonical spellings so
case aliases cannot introduce unchecked files on case-sensitive hosts. Tracked files remain checked when ignored
or deleted. Ignored personal settings are outside this inventory; the checker does not recognize every client's
present or future configuration.

For a client without automatic discovery, explicitly attach or read root `AGENTS.md` at session start. Aider uses
the checked `read` configuration; `/read AGENTS.md` also loads it explicitly. A passing entry-file check cannot
prove that a client loaded or obeyed the rules.

## The crates

One root workspace (`Cargo.toml`): `hal`, `dsp`, `ui`, `fpgad` and `xtask`, with one `Cargo.lock`, one `target/`
and the release profile of the scope binaries. `gpu` is C (`gpu/oos-gpu.c`), built in its own container by
`gpu/build.sh` and `gpu/test-host.sh`, so it is no member. The scope's binaries land in
`target/aarch64-unknown-linux-musl/release/`; the target flags (rust-lld, static, Cortex-A72) are in the root
`.cargo/config.toml`. The gate runs each step crate by crate (`CRATES` in `xtask/src/repo.rs`), so its counts and
ratchets stay per crate; a workspace member without an entry there fails the architecture step. `rustfmt.toml` and
`clippy.toml` sit at the root.

The syntax checks (size, statics, unsafe, module rules) read every file reachable from a member's cargo targets
through its `mod` declarations, inline modules included: library, binaries, integration tests, benches, examples.
They refuse what they cannot follow: `#[path]` on a module (also inside `cfg_attr`) and a `mod x;` inside a function
body. Test code is a test, bench or example target, a `#[cfg(test)]` module (in its own file too), or a `#[test]`
function; a file reached both as test and as production code is production code.

## What each check enforces

| Step | Command | Enforces | Why |
|---|---|---|---|
| fmt | `cargo fmt --check` per crate | `rustfmt.toml`: 120 columns, max heuristics | diffs show changes, not style |
| size limits | syntax walk (`syn`) | file ≤ 400 lines without `#[cfg(test)]` items, fn ≤ 60 lines, struct ≤ 12 fields, a type's inherent impls in ≤ 3 files | small units; the `Ui` of 155 fields over 33 files is what this stops |
| mutable statics | syntax walk | a `static` whose type holds a `Mutex`, `RwLock`, atomic or `Cell`/`RefCell`/`UnsafeCell` (also through the crate's own types) only as a metrics counter (`static` lines in `architecture.txt`); `OnceLock`, `LazyLock` and `OnceCell` around plain data are allowed (set once, then read only) | global mutable state hides ownership |
| unsafe | syntax walk | `unsafe` blocks, functions, impls, traits, extern blocks and attributes (`#[unsafe(..)]`, `no_mangle`, `export_name`, `link_section`) only under the `unsafe` path prefixes of `architecture.txt`: oos-hal and the adapters of ui; `dsp` has `#![forbid(unsafe_code)]` | memory safety stays at the hardware edge (AGENTS.md) |
| disallowed-method exceptions | syntax walk | `#[allow]`/`#[expect]` of `clippy::disallowed_methods` only as listed in `allow.txt` | an exception cannot spread unseen |
| known failures | syntax walk | `#[ignore = "known: task NNNN"]` only as listed in `known.txt`, each naming an open task | a failing test does not hide |
| agent rules | exact loader templates and Git path inventory | the five checked client entry files load root `AGENTS.md`; additional recognized entries fail | universal rules stay in one place |
| architecture | `cargo metadata`, syntax walk | `xtask/data/architecture.txt`: allowed crate dependencies; module rules inside a crate | the layers cannot erode by accident |
| clippy | `cargo clippy --all-targets`, for the host and for `aarch64-unknown-linux-musl` | the `[lints]` table of each crate and `clippy.toml`; the scope build compiles | correctness, idiom, size and complexity, also in Linux-only code |
| tests (debug, release) | `cargo test`, `cargo test --release` | every test passes in both profiles, full counts | release-only bugs (overflow checks, timing) show |
| perf guards | `tools/perf-guard.sh` | the fork's allocation counts and timing ratios; its exit code decides | the fast paths stay fast |
| coverage | `cargo llvm-cov` per crate | line coverage per crate ≥ the floor in `xtask/data/coverage.txt` | coverage does not fall |

The limits have no exemptions (ADR 0010): a record over 12 fields groups its fields into sub-structs.

A function's size is its code lines from `fn` to the closing brace: lines with a token on them, so blank lines,
comments and doc comments do not count (ADR 0010). A file's size is its physical lines outside test code. Test code is
exempt.
Clippy's `too_many_lines` (60 code lines, `clippy.toml`) covers test code too.

The lint set (identical in every crate, `[lints]` in its `Cargo.toml`): `clippy::all`, plus `too_many_lines`,
`cognitive_complexity`, `too_many_arguments`, `type_complexity`, `large_enum_variant`, `undocumented_unsafe_blocks`
(every `unsafe` block has a `// SAFETY:` comment), `missing_safety_doc` and `unwrap_used` (outside
tests: a panic belongs in an `expect` with a reason, or the error is handled). For a `Mutex` use
`oos_hal::sync::lock(&m)`: it takes the data of a poisoned lock, which only a test can produce since the binaries
abort on panic. rustc: the `unused` group, and
`missing_debug_implementations` in the library crates `hal` and `dsp`. Every lint is a warning to rustc and clippy, so
an ordinary build still runs; the gate turns findings into failures through the ratchet. At `deny`, clippy would
stop at the first crate with an old finding and count nothing after it. `xtask` itself denies all of
them and has no ratchet.

The scope's target runs as a second clippy pass per scope crate, on the host: clippy does not link, so the
target's standard library is enough. It compiles the same crates with the same (default) features, target and target
flags (`.cargo/config.toml`) as the image build (`boot/build-dev-image.sh`), without code generation and linking, and
lints the code that only builds there (`cfg(target_os = "linux")`). A finding seen in both passes counts once. The
earlier Docker check mounted every worktree at the same path and could take another tree's build as fresh.

## Ratchets

The rules came in with the code already over them. Those findings are listed, each with its current size or count:

One file per crate and kind, `xtask/data/<crate>/<kind>.txt`, so branches on different crates do not touch the same
file:

- `size.txt`: `<size> <kind> <file or crate> <name>`, every item over a size limit.
- `clippy.txt`: `<count> <crate> <file> <lint>`, every lint finding.
- `statics.txt`: `1 static <file> <name>`, every mutable static that is not a metric.
- `unsafe.txt`: `<count> unsafe <file>`, `unsafe` outside the allowed paths.
- `allow.txt`: `<count> allow <file> clippy::disallowed_methods`, every `#[allow]` or `#[expect]` of the disallowed
  methods (also inside `cfg_attr` and as `#![allow]`), so an adapter-level exception cannot spread unseen.
- `known.txt`: `1 known <file> <test> <task>`, every test ignored as a known failure with
  `#[ignore = "known: task NNNN"]`. The task must be open (in `docs/tasks/`, not in `done/`); an `ignore` reason that
  starts with `known` but names no task fails. Every `#[ignore]` carries a reason, and a `flaky:` reason names its
  issue: `#[ignore = "flaky: #123 races the timer"]`. A bare `#[ignore]` or an untracked flaky test fails.

`xtask/data/coverage.txt` holds the coverage floors, `<floor> <crate>` in tenths of a percent, for all crates.

The gate fails when a listed entry grows, when an entry appears that is not listed (a new oversized function, a new
lint finding in a file), and when coverage falls more than 0.5 points below its floor: measurements move a little
between runs, and the integrator raises the floors after the merge. An entry that shrank passes, and the gate says so.
Write the improvement back:

```sh
cargo xtask ratchet               # lower sizes and counts to the current values, drop fixed entries
cargo xtask ratchet --coverage    # also raise the coverage floors to the current values
```

`ratchet` never raises a size or count and never adds an entry: a new finding is fixed, not listed. Branches do not
edit the ratchet and coverage files: the integrator runs `cargo xtask ratchet` at merge, so two branches never
conflict on them ([migration rules](architecture/migration.md#rules-for-every-step), Shared ratchet files).
`cargo xtask ratchet --init` writes the files from the current state; it only works where the base has no gate.

There is no exception. The upstream rebaseline path is gone with the hard fork
([ADR 0030](adr/process/0030-this-tree-is-a-hard-fork-of-oos.md)); a cherry-picked upstream change passes the gate
like any other.

### Moves

A branch that moves code lists its moves in `xtask/data/moves.txt`, one `<old> -> <new>` per line:

- a key move: both sides are ratchet keys as the gate prints them, `fn ui/src/ui.rs Ui::step -> fn ui/src/step.rs
  Ui::step`;
- a file move: both sides are paths, `ui/src/model.rs -> core/src/model.rs`; it renames the path in every key and the
  crate word of the clippy keys.

The gate then accepts a listed entry under its new key if it is not larger than under the old one at the base. A move
that is not listed makes the entry new, and it fails. A move onto a key that already has an entry fails, and so do two
moves onto one key: merged entries would share the larger allowance. Moves apply in the order listed. A moved entry
may land in a new crate: it brings its allowance with it.

The branch does not edit the ratchet files. At merge the integrator runs `cargo xtask ratchet --move`: it renames the
entries in the ratchet files and keeps in `moves.txt` the moves that still rename an entry at the base. They are the
evidence the integration commit needs against a base without them. A move that names no entry changes nothing, so a
kept move is harmless; the next `ratchet` or `ratchet --move` whose base has the renamed entries drops it. A moved
entry that lands in no crate of `CRATES` fails before anything is written. Plain `cargo xtask ratchet` refuses while a
listed move still names an entry of the ratchet files. The base comparison applies the moves of the base and of the
branch to both sides.

### Base trust

The gate compares each ratchet file with its version at the merge base of `HEAD` and `main` (`origin/main`
in a CI checkout), or `HEAD^1` when `HEAD` is on `main` itself. An entry that is not there, a larger value, or a
removed or lowered coverage floor fails. Any git error fails. `--init` refuses a ratchet file the base already has.

- A ratchet file absent at the base is legal only while the base has no `xtask`: the one merge that brings the gate
  in. After it, a missing ratchet file fails.
- Until 0.2.0, the coordinator's local full gate is authoritative and trusts the `main` ref.
  Setting `CI` or `OOS_GATE_BASE` by hand, or moving `main` to pass a check, breaks the rules.
- On the integration branch, `HEAD^1` covers one commit. A push of several commits is not compared commit by commit:
  the integrator runs the full gate on the branch before the fast-forward.

Lint counts are per file, not per place: fixing one `unwrap` and adding another in the same file passes. That is the
price of a ratchet that survives line moves. Size entries name the item, so they follow it through moves inside the
file; a renamed or moved item counts as new unless `moves.txt` lists it.

## Changing a rule

- A new crate: add it to `CRATES` in `xtask/src/repo.rs` and a `crate` line to `xtask/data/architecture.txt`. It has
  no ratchet files, so it starts clean, except for the entries a planned move brings into it. `ratchet --move` writes
  its files.
- A new dependency: extend its crate's line in `xtask/data/architecture.txt`. The gate fails until the line allows it,
  so the decision is visible in review.
- Module rules in `xtask/data/architecture.txt`, over module paths such as `board` or `board::dho`, and over paths
  into the crate's dependencies under the package name with `_` (`oos_hal::dho`). Every path in the code counts
  (`use` trees, calls, types) in every spelling that names the same item: `crate::`, `self::` and `super::` resolved
  against the file's module and any inline `mod`; a child module named from its parent (`dho::x` in `board`); names
  bound by `use` or `extern crate ... as`, in the module or, for a `use` inside a function, only in its block; glob
  imports (into a dependency always, into a crate module where the name exists); re-exports in any module of the
  crate, followed transitively (`pub use self::dho as hw;` in `board` makes `crate::board::hw` count as
  `board::dho`); `::oos_hal`; a Cargo rename (`hal = { package = "oos-hal" }` makes `hal::dho` count as
  `oos_hal::dho`); raw identifiers (`r#dho` is `dho`). Macro bodies are tokens, not paths: a `confine` rule also
  fails a file whose tokens outside test code spell the confined path (`board::dho`, `oos_hal::dho` or a renamed
  `hal::dho`) outside the allowed modules. Comments and string literals do not count. A module is inside a rule's module when its path starts with
  it segment-wise; for a file, the most specific `layer` rule applies. Test code is free: test targets, test files
  and `#[cfg(test)]` or `#[test]` items.
  - `forbid <crate dir> <from> <to>`: code under `<crate dir>/src/<from>.rs` or `src/<from>/` must not name
    `crate::<to>`.
  - `layer <crate dir> <module> -> <modules...>`: code of `<module>` may name only the listed crate modules and
    itself; nothing after `->` makes it a leaf. Dependency paths are not layer business.
  - `confine <crate dir> <path> -> <modules...>`: `<path>`, a crate module or a dependency path, may be named only from
    inside the listed modules (and a crate module also from inside itself). The board boundary is two of them: only
    `board::dho` uses `oos_hal::dho`, and nothing outside `board` names `board::dho` (`board/mod.rs` selects the
    port).

  Module rules have no ratchet: the tree must meet a rule when it is added. The target rules for `ui` are in
  [Enforcement](architecture/README.md#enforcement); each migration step adds the ones its modules meet.
- The statics check reads the type as written and follows the structs, enums, type aliases and `use ... as`
  renames of the crate it names (not associated types, `<T as Trait>::X`): `OnceLock<Counters>` counts when `Counters` holds atomics, `static S: M<u8>` counts
  after `use std::sync::Mutex as M`. Types match by name, so two types of the same name in one crate count as one.
- A metrics counter as a `static`: `static <file> <name>` in `xtask/data/architecture.txt`, with `<name>` as the
  gate prints it (`<function>::NAME` for a static inside a function). A listed static that no longer exists fails.
- Where `unsafe` may appear: `unsafe <path prefix>` in `xtask/data/architecture.txt` (a crate directory, a
  directory, or a file; `x` also covers `x.rs`). The list moves with the migration.
- A limit: the constants in `xtask/src/size/mod.rs` and the thresholds in `clippy.toml`. Raising a limit lets
  findings go: `cargo xtask ratchet` writes the decreases. Lowering a limit makes findings the ratchet files do not
  list, and they cannot be added (`--init` refuses existing files): the change that lowers a limit fixes them.

## Known limits

Not enforced yet; each is a task.

- Macros: the syntax checks read unexpanded code. Items a `macro_rules!` or `thread_local!` generates (a static, a
  struct, a module path, an `unsafe` block) are not seen. Clippy sees expanded code.
- Module boundaries: an inline `mod` takes the rules of its file's module, not its own; a glob import (`use
  super::*`) followed by a bare name, and a re-export through an allowed module, hide the module the name comes from.
  A path relative to the current module (`dho::x` inside `board`, 2018 edition) is not resolved; nor is a
  dependency path behind a `use oos_hal::dho as d` rename.
- Impl spread counts the spelling of the self type (its last path segment): aliases split one type, and two types of
  the same name merge.
- Path dependencies are checked by package name only, not by source: a local fork of an allowed crate passes.
- Types imported from another workspace crate, and associated types, are not followed by the statics check.
- The base branch is the local `main` ref; the coordinator validates it while repository CI is off.
- pre-commit checks the working tree, not the staged index, and only the fast part. The done rule is the full gate
  on the final commit.
